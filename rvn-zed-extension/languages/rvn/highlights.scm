[
  "label"
  "scene"
  "choice"
  "jump"
  "call"
  "set"
  "if"
  "else"
  "use"
  "init"
  "cinematic"
  "unlock_ending"
  "imagemap"
  "hotspot"
] @keyword

(return_statement) @keyword.return

(cinematic_statement
  "hide" @keyword)

[
  "with"
  "at"
] @operator

[
  "and"
  "or"
  "not"
] @operator

(comment) @comment
(string) @string
(string_content) @string
(escape_sequence) @string.escape
(text_tag) @string.special
(number) @number
(boolean) @boolean

(label_statement
  name: (label_name
    (name) @label))

(jump_statement
  target: (label_reference
    (name) @label))

(call_statement
  target: (label_reference
    (name) @label))

(set_statement
  name: (variable_name
    (name) @variable.definition))

(identifier) @variable

(unlock_ending_statement
  name: [(identifier) (string)] @label)

(cinematic_statement
  name: [(identifier) (string)] @label)

(choice_option
  label: (string) @string.special)

(dialogue_statement
  character: (character_name
    (name) @variable.special)
  text: (string) @string)

(narration_statement
  text: (string) @string.special)

(method_call_expression
  target: (command_target
    (name) @variable.special)
  "." @punctuation.delimiter
  method: (method_name
    (name) @function.method))

(property_statement
  key: (property_name
    (name) @property))

(transition
  (transition_name
    (name) @function))

(position_name) @constant

(interpolation
  "[" @punctuation.special
  "]" @punctuation.special) @embedded

(interpolation
  (expression
    (identifier) @variable))

[
  "=>"
  "="
  ":"
  "."
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "+"
  "-"
  "*"
  "/"
] @operator

[
  "{"
  "}"
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

"," @punctuation.delimiter
