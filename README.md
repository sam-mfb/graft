# game-localizer

Tools for creating and applying binary patches to game files.

## Installation

```
cargo install --path .
```

## Commands

### Diff

Create a diff:
```
game-localizer diff create <original> <modified> <diff-output>
```

Apply a diff:
```
game-localizer diff apply <original> <diff-file> <output>
```

### Hash

Calculate SHA-256 hash of a file:
```
game-localizer hash calculate <file>
```

Compare two files by hash:
```
game-localizer hash compare <file1> <file2>
```

Check if a file matches an expected hash:
```
game-localizer hash check <hash> <file>
```
