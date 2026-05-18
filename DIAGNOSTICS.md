# llmlang Diagnostic Codes

This document maps the token-efficient diagnostic codes emitted by the compiler to human-readable explanations.

## Errors (E)
- **E000**: Unexpected EOF during parsing.
- **E001**: Unexpected token encountered.
- **E002**: Expected identifier after definition marker (`:` or `#`).
- **E003**: De Bruijn index out of bounds.
- **E004**: Variable already moved; cannot access.
- **E005**: Cannot move variable; already moved.
- **E006**: Unknown shape name.
- **E007**: Field not found in shape.
- **E008**: Unsupported binary operation.
- **E009**: Branch stack state mismatch (linear typing violation).
- **E010**: Unknown function in Apply operation.
- **E011**: Function call returned void or invalid value.
- **E012**: Only named function calls supported in Apply.
- **E013**: Unknown identifier.

## Warnings (W)
- **W001**: Variable leaked; defined but never consumed (linear typing violation).
