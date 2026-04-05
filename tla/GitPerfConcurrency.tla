--------------------------- MODULE GitPerfConcurrency ---------------------------
(*
 * TLA+ specification of the git-perf concurrent add/push/remove protocol.
 *
 * Formally verifies the core safety property:
 *   "No measurement successfully written to a write-ref and not targeted
 *    for removal is ever silently dropped from the system."
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
 * REMOVE  (execute_notes_operation, called by raw_remove_measurements_from_commits):
 *   R1. git fetch  (upstream := remote)
 *   R2. Capture U = current upstream OID (refs/notes/perf-v3)
 *   R3. Create temp rewrite-ref T at U  (refs/notes/perf-v3-rewrite-<random>)
 *         T is NOT in refs/notes/perf-v3-write-*, so pushers never enumerate it
 *   R4. Apply removal: remove notes for commits older than threshold from T
 *         (git notes --ref T remove --stdin --ignore-missing)
 *         modelled as: T := U \ RemovableMeasurements
 *   R5. Compact T: create orphan commit with same tree (cuts history, no content change)
 *         no-op in terms of content; omitted from this model
 *   R6. git push --force-with-lease T  (CAS: remote == U → remote := T)
 *         on conflict → exponential-backoff retry from R1
 *   R7. git update-ref (unconditional): upstream := T
 *   R8. Delete T  (cleanup; no functional effect on safety)
 *
 * -----------------------------------------------------------------------
 * Why the remove protocol is safe for non-removable measurements
 * -----------------------------------------------------------------------
 * The remove operation targets only measurements already consolidated in
 * refs/notes/perf-v3 (upstream/remote).  Write-refs are never touched.
 * Two safety-critical interleavings:
 *
 *  Case 1 – concurrent pusher updates remote BEFORE remover's CAS push (R6):
 *    The remover's CAS check (remote == U) fails.
 *    The remover retries from R1, re-fetches the new remote, and applies
 *    removal to the updated state.  ✓
 *
 *  Case 2 – concurrent adder commits to a write-ref during the remove:
 *    The adder writes to a write-ref (not to upstream/remote).
 *    The remove only modifies upstream/remote; write-refs are untouched.
 *    The next push will merge the write-ref into the post-remove remote,
 *    so the measurement is preserved.  ✓
 *
 * Note: R7 is an unconditional upstream update (not a CAS).  If another
 * pusher has already advanced upstream between R6 and R7, upstream is
 * transiently rolled back to the post-remove value.  This is safe because:
 *  - Any pusher that subsequently captures U and tries to push will see
 *    a CAS failure (remote ≠ U) and retry with the correct upstream.
 *  - Measurements in write-refs are never lost: they survive until a
 *    pusher successfully merges them into remote.
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
 * - Remover steps R3–R4 are combined into one atomic action (ApplyRemoval)
 *   since the rewrite-ref is a local intermediate invisible to other processes
 *   (different ref prefix, never enumerated by pushers).
 * - Step R5 (compact) is omitted: it does not change content.
 * - Step R8 (delete T) is omitted: a cleaned-up ref has no safety relevance.
 * - RemovableMeasurements is a constant: the set of measurements the remover
 *   is authorised to delete.  The invariant only protects measurements
 *   outside this set.
 *)
EXTENDS Integers, FiniteSets, TLC

CONSTANTS
    Adders,               \* Set of adder process identifiers
    Pushers,              \* Set of pusher process identifiers
    Removers,             \* Set of remover process identifiers (may be empty)
    RemovableMeasurements, \* Measurements the remover is authorised to delete
    NonExistent           \* Model-value sentinel: ref does not exist as a git object

ASSUME /\ Adders  # {}
       /\ Pushers # {}
       /\ Adders  \cap Pushers  = {}
       /\ Removers \cap Adders  = {}
       /\ Removers \cap Pushers = {}

\* Each adder contributes exactly one measurement: its own ID.
Measurement(a) == a
AllMeasurements == { Measurement(a) : a \in Adders }

\* RemovableMeasurements must be a subset of the measurements that adders produce.
ASSUME RemovableMeasurements \subseteq AllMeasurements

\* Finite pool of write-ref IDs (natural numbers).
\* Sized conservatively to cover all refs created during TLC model checking.
\*   1 initial write-target
\*   + per pusher attempt: 1 W_new (write-target) + 1 merge-ref
\*   + per adder: retries bounded by MaxWR
\* Removers do not allocate write-refs (their rewrite-ref is modelled as
\* local state), so they do not increase the pool size.
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
    pusherCaptured,  \* Pushers -> [WRPool -> SUBSET AllMeasurements]:
                     \*   snapshot of write-target content taken at P4

    \* --- Per-remover local state ---
    removerPC,       \* Removers -> {"Fetch","ApplyRemoval","PushRemote",
                     \*              "UpdateLocal","Done"}
    removerU,        \* Removers -> SUBSET AllMeasurements: upstream captured at R2
    removerT         \* Removers -> SUBSET AllMeasurements: filtered content (U \ removable)
                     \*   models the temp rewrite-ref (refs/notes/perf-v3-rewrite-*)
                     \*   which is local to the remover and invisible to pushers

vars == <<writeRefs, symrefTarget, upstream, remote, nextWR,
          writeTargetIds,
          adderPC, adderT, adderH, adderCommitted,
          pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
          removerPC, removerU, removerT>>

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
    /\ removerPC      = [r \in Removers |-> "Fetch"]
    /\ removerU       = [r \in Removers |-> {}]
    /\ removerT       = [r \in Removers |-> {}]

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
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

\* P2: Capture U = current upstream.
\*     Separate step so a concurrent fetch from another pusher can make
\*     U stale before the CAS-create of the merge-ref (P3).
PusherCaptureU(p) ==
    /\ pusherPC[p] = "CaptureU"
    /\ pusherU'   = [pusherU  EXCEPT ![p] = upstream]
    /\ pusherPC'  = [pusherPC EXCEPT ![p] = "CreateMergeRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherNewWR, pusherU, pusherCaptured,
                   removerPC, removerU, removerT>>

\* P3': CAS failed – upstream changed since P2 – fetch then retry.
PusherCreateMergeRefFail(p) ==
    /\ pusherPC[p] = "CreateMergeRef"
    /\ upstream # pusherU[p]
    /\ upstream' = remote              \* Fetch
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "NewWriteRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherNewWR, pusherU, pusherMergeRef,
                   removerPC, removerU, removerT>>

\* P6: git push --force-with-lease  (CAS: remote == U → remote := content(M)).
PusherPushSuccess(p) ==
    /\ pusherPC[p] = "PushRemote"
    /\ remote = pusherU[p]             \* CAS: remote still at U
    /\ remote'   = writeRefs[pusherMergeRef[p]]
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "Fetch"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

\* P6': Push failed – another pusher already updated remote – fetch + retry.
PusherPushFail(p) ==
    /\ pusherPC[p] = "PushRemote"
    /\ remote # pusherU[p]
    /\ upstream' = remote              \* Fetch
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "NewWriteRef"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

\* P7: Fetch – update local upstream from remote.
PusherFetch(p) ==
    /\ pusherPC[p] = "Fetch"
    /\ upstream' = remote
    /\ pusherPC' = [pusherPC EXCEPT ![p] = "DeleteRefs"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds, adderPC, adderT, adderH, adderCommitted,
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

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
                   pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerPC, removerU, removerT>>

(* ====================================================================
   REMOVER ACTIONS
   ==================================================================== *)

\* R1: Fetch – update local upstream from remote.
\*     In the real code this is pull_internal() at the start of
\*     execute_notes_operation (and on every backoff retry).
RemoverFetch(r) ==
    /\ removerPC[r] = "Fetch"
    /\ upstream'   = remote
    /\ removerPC'  = [removerPC EXCEPT ![r] = "ApplyRemoval"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds,
                   adderPC, adderT, adderH, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerU, removerT>>

\* R2 + R3 + R4 (combined, modelled atomically):
\*   R2: Capture U = current upstream (after fetch)
\*   R3: Create temp rewrite-ref T at U (refs/notes/perf-v3-rewrite-<random>)
\*       Modelled as local remover state: T is invisible to pushers because
\*       it uses a different ref prefix (not refs/notes/perf-v3-write-*) and
\*       is therefore never enumerated by PusherEnumMerge.
\*   R4: Apply removal: T := U \ RemovableMeasurements
\*       (git notes --ref T remove --stdin --ignore-missing for old commits)
\* R5 (compact) is omitted: it is a history-rewriting no-op that does not
\*       change the content set, so it has no safety relevance.
RemoverApplyRemoval(r) ==
    /\ removerPC[r] = "ApplyRemoval"
    /\ removerU'   = [removerU EXCEPT ![r] = upstream]
    /\ removerT'   = [removerT EXCEPT ![r] = upstream \ RemovableMeasurements]
    /\ removerPC'  = [removerPC EXCEPT ![r] = "PushRemote"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, remote, nextWR,
                   writeTargetIds,
                   adderPC, adderT, adderH, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured>>

\* R6: git push --force-with-lease  (CAS: remote == U → remote := T).
RemoverPushSuccess(r) ==
    /\ removerPC[r] = "PushRemote"
    /\ remote = removerU[r]              \* CAS: remote still at U
    /\ remote'    = removerT[r]
    /\ removerPC' = [removerPC EXCEPT ![r] = "UpdateLocal"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, nextWR,
                   writeTargetIds,
                   adderPC, adderT, adderH, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerU, removerT>>

\* R6': Push failed – remote changed since R2 – retry from fetch.
\*      In the real code the backoff retry re-invokes execute_notes_operation
\*      which calls pull_internal first, so we return to "Fetch" state.
RemoverPushFail(r) ==
    /\ removerPC[r] = "PushRemote"
    /\ remote # removerU[r]
    /\ removerPC' = [removerPC EXCEPT ![r] = "Fetch"]
    /\ UNCHANGED <<writeRefs, symrefTarget, upstream, remote, nextWR,
                   writeTargetIds,
                   adderPC, adderT, adderH, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerU, removerT>>

\* R7: Unconditional local update: upstream := T.
\*     In the real code: git update-ref (no CAS) refs/notes/perf-v3 <target>
\*     This can transiently roll back upstream if a concurrent pusher has
\*     already advanced it, but all push operations protect data via their own
\*     CAS checks, so no measurement is lost.
RemoverUpdateLocal(r) ==
    /\ removerPC[r] = "UpdateLocal"
    /\ upstream'   = removerT[r]
    /\ removerPC'  = [removerPC EXCEPT ![r] = "Done"]
    /\ UNCHANGED <<writeRefs, symrefTarget, remote, nextWR,
                   writeTargetIds,
                   adderPC, adderT, adderH, adderCommitted,
                   pusherPC, pusherNewWR, pusherU, pusherMergeRef, pusherCaptured,
                   removerU, removerT>>

(* ====================================================================
   TRANSITION RELATION
   ==================================================================== *)
\* Terminal state: all processes have finished.  This self-loop lets TLC
\* know the state is intentionally terminal rather than an unexpected
\* deadlock.
Terminating ==
    /\ \A a \in Adders   : adderPC[a]   = "Done"
    /\ \A p \in Pushers  : pusherPC[p]  = "Done"
    /\ \A r \in Removers : removerPC[r] = "Done"
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
    \/ \E r \in Removers :
           \/ RemoverFetch(r)
           \/ RemoverApplyRemoval(r)
           \/ RemoverPushSuccess(r)
           \/ RemoverPushFail(r)
           \/ RemoverUpdateLocal(r)
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
\* For every adder a: if a's CAS write ever succeeded (adderCommitted[a])
\* AND the measurement is not authorised for removal (not in RemovableMeasurements),
\* then Measurement(a) is either:
\*   (a) still present in some write-ref (waiting to be pushed), OR
\*   (b) already present in remote (successfully pushed)
\*
\* Measurements in RemovableMeasurements may legitimately be deleted by a
\* remover process, so they are excluded from this guarantee.
\* Note: a measurement present in BOTH a write-ref and remote also satisfies
\* the invariant (idempotent delivery is fine).
\* -----------------------------------------------------------------------
NoMeasurementLost ==
    \A a \in Adders :
        /\ adderCommitted[a]
        /\ Measurement(a) \notin RemovableMeasurements
        =>
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
