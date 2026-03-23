--------------------------- MODULE GitPerfConcurrency ---------------------------
(*
 * TLA+ specification of the git-perf concurrent add/push protocol.
 *
 * Formally verifies the core safety property:
 *   "No measurement successfully written to a write-ref is ever
 *    silently dropped from the system."
 *
 * -----------------------------------------------------------------------
 * Background
 * -----------------------------------------------------------------------
 * git-perf stores performance measurements as git notes in the branch
 * refs/notes/perf-v3.  To allow concurrent local adds without per-process
 * locking, it uses a set of temporary "write-target" refs
 * (refs/notes/perf-v3-write-<random>).  A symbolic ref
 * (refs/notes/perf-v3-write) indirects to the currently-active target.
 *
 * -----------------------------------------------------------------------
 * Protocol (from git_perf/src/git/git_interop.rs)
 * -----------------------------------------------------------------------
 *
 * ADD  (raw_add_note_line):
 *   A1. Read current symref target T and its content H
 *       (two separate git commands – not atomic together)
 *   A2. Append measurement m to a temp-add-ref: content = H ∪ {m}
 *   A3. git update-ref CAS:  update T  new_oid  old_oid
 *         succeeds iff T's current OID still equals old_oid (= H)
 *         on conflict (OID mismatch) → retry from A1
 *
 * PUSH  (raw_push, wrapped with exponential-backoff retry + fetch):
 *   P1. Redirect symref to fresh write-target W_new  [non-atomic]
 *   P2. Capture U = upstream (refs/notes/perf-v3 local)
 *   P3. Atomic transaction: verify upstream == U
 *                            create merge-ref M at U
 *         on conflict → fetch then retry from P1
 *   P4. git for-each-ref: enumerate all refs/notes/perf-v3-write-* except W_new
 *         record each as (refname → OID/content) → captured
 *   P5. For each ref in captured: git notes merge -s cat_sort_uniq <captured-OID>
 *         merges the content AT THE CAPTURED OID into M (set union)
 *   P6. git push --force-with-lease M  (CAS: remote == U → remote := M)
 *         on conflict → fetch then retry from P1
 *   P7. git fetch  (upstream := remote)
 *   P8. Atomic batch: delete each ref in captured iff current OID == captured OID
 *         on any mismatch → whole transaction aborted → fetch then retry from P1
 *
 * -----------------------------------------------------------------------
 * Why the protocol is safe (answers the '??' comment in the source)
 * -----------------------------------------------------------------------
 * The question is: what happens when an adder concurrently writes to the
 * old write-target AFTER the pusher has already redirected the symref?
 *
 * There are two cases depending on when the adder's CAS fires relative
 * to the pusher's enumerate step (P4):
 *
 *  Case 1 – adder CAS fires BEFORE P4:
 *    The ref appears in captured with the adder's new content.
 *    The pusher merges it and pushes it to remote.  ✓
 *
 *  Case 2 – adder CAS fires AFTER P4 (but before P8):
 *    The ref's OID has changed since it was captured.
 *    The pusher's batch delete (P8) fails the CAS check for that ref.
 *    The pusher retries from P1, re-enumerates the ref with the new
 *    content, and includes the measurement in the next push.  ✓
 *
 * In both cases the measurement is never silently discarded.
 *
 * -----------------------------------------------------------------------
 * Modelling choices
 * -----------------------------------------------------------------------
 * - Measurements are opaque values; each adder adds exactly one (its ID).
 * - git notes content is a set of measurements (cat_sort_uniq = set union).
 * - OIDs are identified with content sets: equal content <-> equal OID.
 * - Write-ref names are drawn from a finite pool of integers.
 * - Merge-refs (refs/notes/perf-v3-merge-STAR) are tracked separately so
 *   they are never confused with write-targets during enumeration.
 * - Only safety is checked; liveness / termination are out of scope.
 * - Steps P4 and P5 are modelled as one atomic action (overapproximation)
 *   because combining them cannot hide safety violations: the delete CAS
 *   is the true guard that prevents measurement loss.
 *)
EXTENDS Integers, FiniteSets, TLC

CONSTANTS
    Adders,      \* Set of adder process identifiers
    Pushers,     \* Set of pusher process identifiers
    NonExistent  \* Model-value sentinel: ref does not exist as a git object

ASSUME /\ Adders  # {}
       /\ Pushers # {}
       /\ Adders \cap Pushers = {}

\* Each adder contributes exactly one measurement: its own ID.
Measurement(a) == a
AllMeasurements == { Measurement(a) : a \in Adders }

\* Finite pool of write-ref IDs (natural numbers).
\* Sized conservatively to cover all refs created during TLC model checking.
\*   1 initial write-target
\*   + per pusher attempt: 1 W_new (write-target) + 1 merge-ref
\*   + per adder: retries bounded by MaxWR
MaxWR == 2 + 3 * Cardinality(Adders) + 4 * Cardinality(Pushers)
WRPool == 1..MaxWR

(* ====================================================================
   STATE VARIABLES
   ==================================================================== *)
VARIABLES
    \* --- Git repository state ---
    writeRefs,      \* WRPool -> (SUBSET AllMeasurements) | NonExistent
    symrefTarget,   \* WRPool: write-target the symbolic ref points to
    upstream,       \* SUBSET AllMeasurements: local refs/notes/perf-v3
    remote,         \* SUBSET AllMeasurements: remote refs/notes/perf-v3
    nextWR,         \* WRPool: next ref ID to allocate

    \* --- Ref-type bookkeeping ---
    \* write-targets (refs/notes/perf-v3-write-*) are enumerable by pushers;
    \* merge-refs (refs/notes/perf-v3-merge-*) are not.
    writeTargetIds, \* SUBSET WRPool: IDs that are write-target refs

    \* --- Per-adder local state ---
    adderPC,         \* Adders -> {"ReadSymref","CASWrite","Done"}
    adderT,          \* Adders -> WRPool: captured symref target
    adderH,          \* Adders -> SUBSET AllMeasurements: captured content of T
    adderCommitted,  \* Adders -> BOOLEAN: has the adder's CAS ever succeeded?

    \* --- Per-pusher local state ---
    pusherPC,        \* Pushers -> {"NewWriteRef","CaptureU","CreateMergeRef",
                     \*             "EnumMerge","PushRemote","Fetch",
                     \*             "DeleteRefs","Done"}
    pusherNewWR,     \* Pushers -> WRPool: W_new created in P1
    pusherU,         \* Pushers -> SUBSET AllMeasurements: captured upstream
    pusherMergeRef,  \* Pushers -> WRPool | 0: merge-ref (0 = not yet created)
    pusherCaptured   \* Pushers -> [WRPool -> SUBSET AllMeasurements]:
                     \*   snapshot of write-target content taken at P4

vars == <<writeRefs, symrefTarget, upstream, remote, nextWR,
          writeTargetIds,
          adderPC, adderT, adderH, adderCommitted,
          pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

(* ====================================================================
   HELPERS
   ==================================================================== *)

\* Content of a write-ref (empty set if the ref does not exist)
WRContent(wr) ==
    IF writeRefs[wr] = NonExistent THEN {} ELSE writeRefs[wr]

\* True iff the ref has been created (exists as a git object or empty marker)
WRLive(wr) ==
    writeRefs[wr] # NonExistent

\* Write-target refs eligible for enumeration by a pusher:
\*   - must be a write-target (not a merge-ref)
\*   - must have non-empty content (git for-each-ref only returns live objects)
EnumerableWRs ==
    { wr \in writeTargetIds : WRLive(wr) /\ WRContent(wr) # {} }

(* ====================================================================
   INITIAL STATE
   ==================================================================== *)
Init ==
    \* One empty write-target exists; the symref points to it.
    /\ writeRefs      = [wr \in WRPool |-> IF wr = 1 THEN {} ELSE NonExistent]
    /\ symrefTarget   = 1
    /\ upstream       = {}
    /\ remote         = {}
    /\ nextWR         = 2
    /\ writeTargetIds = {1}
    /\ adderPC        = [a \in Adders  |-> "ReadSymref"]
    /\ adderT         = [a \in Adders  |-> 1]
    /\ adderH         = [a \in Adders  |-> {}]
    /\ adderCommitted = [a \in Adders  |-> FALSE]
    /\ pusherPC       = [p \in Pushers |-> "NewWriteRef"]
    /\ pusherNewWR    = [p \in Pushers |-> 0]
    /\ pusherU        = [p \in Pushers |-> {}]
    /\ pusherMergeRef = [p \in Pushers |-> 0]
    /\ pusherCaptured = [p \in Pushers |-> [wr \in {} |-> {}]]

(* ====================================================================
   ADDER ACTIONS
   ==================================================================== *)

\* A1: Read current symref target T and its content H.
\*     These are two separate git commands in the real code; we model
\*     them as one step since both are reads and any write that races
\*     between them is captured by the CAS step below.
AdderReadSymref(a) ==
    /\ adderPC[a] = "ReadSymref"
    /\ adderT'    = [adderT EXCEPT ![a] = symrefTarget]
    /\ adderH'    = [adderH EXCEPT ![a] = WRContent(symrefTarget)]
    /\ adderPC'   = [adderPC EXCEPT ![a] = "CASWrite"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* A2 + A3: Write measurement to T via CAS.
\*   Real code: create temp-add-ref (copy of H plus m), then update-ref CAS.
\*   We compress both into one step since the temp-add-ref is a local
\*   intermediate that is always cleaned up and never observed by others.
\*   Precondition: T still has content H (CAS check).
AdderCASWrite(a) ==
    /\ adderPC[a] = "CASWrite"
    /\ LET t   == adderT[a]
           h   == adderH[a]
           new == h \cup {Measurement(a)}
       IN
       /\ WRLive(t)            \* T must still exist
       /\ WRContent(t) = h     \* CAS: T unchanged since read
       /\ writeRefs'      = [writeRefs      EXCEPT ![t] = new]
       /\ adderCommitted' = [adderCommitted EXCEPT ![a] = TRUE]
       /\ adderPC'        = [adderPC        EXCEPT ![a] = "Done"]
    /\ UNCHANGED <<symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderT, adderH,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* A3': CAS conflict – T changed or was deleted – retry from A1.
AdderCASFail(a) ==
    /\ adderPC[a] = "CASWrite"
    /\ LET t == adderT[a]
           h == adderH[a]
       IN \/ ~WRLive(t)
          \/ WRContent(t) # h
    /\ adderPC' = [adderPC EXCEPT ![a] = "ReadSymref"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderT, adderH, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

(* ====================================================================
   PUSHER ACTIONS
   ==================================================================== *)

\* P1: Create fresh write-target W_new and redirect the symref to it.
\*     Non-atomic: an adder that already read the old symrefTarget will
\*     still write there (it's captured by the enumerate step later).
PusherNewWriteRef(p) ==
    /\ pusherPC[p] = "NewWriteRef"
    /\ nextWR \in WRPool
    /\ LET wn == nextWR IN
       /\ writeRefs'      = [writeRefs      EXCEPT ![wn] = {}]
       /\ symrefTarget'   = wn
       /\ writeTargetIds' = writeTargetIds \cup {wn}
       /\ pusherNewWR'    = [pusherNewWR    EXCEPT ![p]  = wn]
       /\ nextWR'         = nextWR + 1
       /\ pusherPC'       = [pusherPC       EXCEPT ![p]  = "CaptureU"]
    /\ UNCHANGED <<upstream, remote,
                   adderPC, adderT, adderH, adderCommitted,
                   pusherU, pusherMergeRef, pusherCaptured>>

\* P2: Capture U = current upstream.
\*     Separate step so a concurrent fetch from another pusher can make
\*     U stale before the CAS-create of the merge-ref (P3).
PusherCaptureU(p) ==
    /\ pusherPC[p] = "CaptureU"
    /\ pusherU'   = [pusherU  EXCEPT ![p] = upstream]
    /\ pusherPC'  = [pusherPC EXCEPT ![p] = "CreateMergeRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherMergeRef, pusherCaptured>>

\* P3: Atomic transaction: verify upstream == U AND create merge-ref M at U.
\*     Fails if upstream has been updated by a concurrent fetch.
PusherCreateMergeRef(p) ==
    /\ pusherPC[p] = "CreateMergeRef"
    /\ nextWR \in WRPool
    /\ upstream = pusherU[p]           \* CAS: upstream still matches captured U
    /\ LET mr == nextWR IN
       \* Note: mr is NOT added to writeTargetIds (it is a merge-ref)
       /\ writeRefs'      = [writeRefs      EXCEPT ![mr] = upstream]
       /\ pusherMergeRef' = [pusherMergeRef EXCEPT ![p]  = mr]
       /\ nextWR'         = nextWR + 1
       /\ pusherPC'       = [pusherPC       EXCEPT ![p]  = "EnumMerge"]
    /\ UNCHANGED <<symrefTarget, upstream, remote,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherCaptured>>

\* P3': CAS failed – upstream changed since P2 – fetch then retry.
PusherCreateMergeRefFail(p) ==
    /\ pusherPC[p] = "CreateMergeRef"
    /\ upstream # pusherU[p]
    /\ upstream' = remote              \* Fetch
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "NewWriteRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* P4 + P5 (combined, modelled atomically):
\*   Enumerate all write-target refs except W_new and the merge-ref M,
\*   record their current content as captured OIDs, and merge all into M.
\*
\*   Real code: git for-each-ref (P4) then a loop of git notes merge (P5).
\*   In reality these are non-atomic; we model them together as an
\*   overapproximation (more content merged → can only help safety).
\*   The CAS-delete in P8 is the true guard against data loss.
\*
\*   Key: only refs with non-empty content are enumerated (git for-each-ref
\*   returns nothing for refs that point to no git object).
PusherEnumMerge(p) ==
    /\ pusherPC[p] = "EnumMerge"
    /\ LET wn  == pusherNewWR[p]
           mr  == pusherMergeRef[p]
           cap == { wr \in EnumerableWRs : wr # wn /\ wr # mr }
           captured    == [wr \in cap |-> writeRefs[wr]]
           mergedContent == UNION ({ writeRefs[wr] : wr \in cap }
                                   \cup {writeRefs[mr]})
       IN
       /\ writeRefs'      = [writeRefs      EXCEPT ![mr] = mergedContent]
       /\ pusherCaptured' = [pusherCaptured EXCEPT ![p]  = captured]
       /\ pusherPC'       = [pusherPC       EXCEPT ![p]  = "PushRemote"]
    /\ UNCHANGED <<symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef>>

\* P6: git push --force-with-lease  (CAS: remote == U → remote := content(M)).
PusherPushSuccess(p) ==
    /\ pusherPC[p] = "PushRemote"
    /\ remote = pusherU[p]             \* CAS: remote still at U
    /\ remote'   = writeRefs[pusherMergeRef[p]]
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "Fetch"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* P6': Push failed – another pusher already updated remote – fetch + retry.
PusherPushFail(p) ==
    /\ pusherPC[p] = "PushRemote"
    /\ remote # pusherU[p]
    /\ upstream' = remote              \* Fetch
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "NewWriteRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* P7: Fetch – update local upstream from remote.
PusherFetch(p) ==
    /\ pusherPC[p] = "Fetch"
    /\ upstream' = remote
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "DeleteRefs"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* P8: Atomic batch CAS-delete of all captured write-refs.
\*   git update-ref transaction:  delete <refname> <oid>  for each captured ref.
\*   All-or-nothing: if ANY ref's current OID differs from its captured OID
\*   (an adder wrote to it after P4), the entire transaction is aborted.
\*
\*   SUCCESS branch: all CAS checks pass → delete captured refs.
PusherDeleteSuccess(p) ==
    /\ pusherPC[p] = "DeleteRefs"
    /\ LET cap == pusherCaptured[p] IN
       /\ \A wr \in DOMAIN cap : WRContent(wr) = cap[wr]   \* All CAS checks pass
       /\ writeRefs' = [wr \in WRPool |->
                           IF wr \in DOMAIN cap
                           THEN NonExistent
                           ELSE writeRefs[wr]]
       /\ pusherPC'  = [pusherPC EXCEPT ![p] = "Done"]
    /\ UNCHANGED <<symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* P8': CAS-delete failed – at least one ref changed since P4 – fetch + retry.
\*   (The measurement that changed the ref is still in the system; the next
\*   push attempt will pick it up.)
PusherDeleteFail(p) ==
    /\ pusherPC[p] = "DeleteRefs"
    /\ LET cap == pusherCaptured[p] IN
       /\ \E wr \in DOMAIN cap : WRContent(wr) # cap[wr]
    /\ upstream' = remote              \* Fetch
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "NewWriteRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

(* ====================================================================
   TRANSITION RELATION
   ==================================================================== *)
\* Terminal state: all processes have finished.  This self-loop lets TLC
\* know the state is intentionally terminal rather than an unexpected
\* deadlock.
Terminating ==
    /\ \A a \in Adders  : adderPC[a]  = "Done"
    /\ \A p \in Pushers : pusherPC[p] = "Done"
    /\ UNCHANGED vars

Next ==
    \/ \E a \in Adders :
           \/ AdderReadSymref(a)
           \/ AdderCASWrite(a)
           \/ AdderCASFail(a)
    \/ \E p \in Pushers :
           \/ PusherNewWriteRef(p)
           \/ PusherCaptureU(p)
           \/ PusherCreateMergeRef(p)
           \/ PusherCreateMergeRefFail(p)
           \/ PusherEnumMerge(p)
           \/ PusherPushSuccess(p)
           \/ PusherPushFail(p)
           \/ PusherFetch(p)
           \/ PusherDeleteSuccess(p)
           \/ PusherDeleteFail(p)
    \/ Terminating

(* ====================================================================
   SAFETY INVARIANTS
   ==================================================================== *)

\* All measurements currently held in any write-ref
MeasurementsInWriteRefs ==
    UNION { WRContent(wr) : wr \in WRPool }

\* -----------------------------------------------------------------------
\* MAIN SAFETY INVARIANT: NoMeasurementLost
\*
\* For every adder a: if a's CAS write ever succeeded (adderCommitted[a]),
\* then Measurement(a) is either:
\*   (a) still present in some write-ref (waiting to be pushed), OR
\*   (b) already present in remote (successfully pushed)
\*
\* This is the formal statement that no committed measurement is silently
\* dropped.  Note: a measurement that is present in BOTH a write-ref and
\* remote also satisfies the invariant (idempotent delivery is fine).
\* -----------------------------------------------------------------------
NoMeasurementLost ==
    \A a \in Adders :
        adderCommitted[a] =>
            \/ Measurement(a) \in remote
            \/ Measurement(a) \in MeasurementsInWriteRefs

\* The symbolic ref always points to a live (created) write-ref
SymrefValid ==
    WRLive(symrefTarget) /\ symrefTarget \in writeTargetIds

\* No write-ref contains a measurement not belonging to any adder
WriteRefsHaveValidContent ==
    \A wr \in WRPool : WRLive(wr) => writeRefs[wr] \subseteq AllMeasurements

\* The remote only contains valid measurements
RemoteHasValidContent ==
    remote \subseteq AllMeasurements

\* The local upstream only contains valid measurements
UpstreamHasValidContent ==
    upstream \subseteq AllMeasurements

\* Combined invariant checked by TLC
Invariant ==
    /\ NoMeasurementLost
    /\ SymrefValid
    /\ WriteRefsHaveValidContent
    /\ RemoteHasValidContent
    /\ UpstreamHasValidContent

(* ====================================================================
   SPECIFICATION
   ==================================================================== *)
Spec == Init /\ [][Next]_vars

THEOREM Spec => []Invariant

=============================================================================
