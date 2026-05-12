; Comments
(comment) @comment

; Mapping keys (includes the trailing colon)
(mapping_key) @property

; Literals
(null_literal) @constant.builtin
(boolean_literal) @constant.builtin
(integer_literal) @number

; Strings
(double_quoted_string) @string
(triple_quoted_string) @string
(bare_string) @string
(flow_bare_string) @string

; Punctuation
["," ] @punctuation.delimiter
(sequence_indicator) @punctuation.delimiter
["[" "]"] @punctuation.bracket
["{" "}"] @punctuation.bracket
