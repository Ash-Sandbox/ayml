AYML is a simplified YAML variant. See README.md for design goals and basic commands.

Treat every request as though you own the whole codebase and can make any change, anywhere.
There is zero legacy code to maintain, so every task MUST be approached from first principles.
If you see an approach that is subpar, prefer to rip it out wholesale and replace it with the better alternative.

## Comment Guidelines

- Doc comments should explain an API for the benefit of the caller.
- Code comments should be used only to explain unintuitive code. Whenever you add a code comment,
  you should ask yourself whether the comment is reflective of poorly written code or whether it
  is an expression of code that is so intrinsically complex that a comment is the only way to make
  the code understandable.
- Avoid changelog-style comments at all cost. They pollute the codebase and harm the reader's
  understanding of the code as it exists.

## Doc Guidelines

- If you find redundancies or overly verbose phrases while reading through a doc, remove them.
- LLM written docs tend to turn into factslop over time. If you find that a doc seems to contain
  a lot of facts but takes too many words to communicate an idea, restructure it to explain
  exactly what you were looking for, concisely.
