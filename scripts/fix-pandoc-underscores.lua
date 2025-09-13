-- Lua filter to fix underscore escaping in pandoc output
-- This prevents underscores from being escaped unnecessarily

function Str(el)
  -- Remove backslash escaping from underscores
  el.text = el.text:gsub("\\_", "_")
  return el
end

function Code(el)
  -- Also handle underscores in code blocks
  el.text = el.text:gsub("\\_", "_")
  return el
end