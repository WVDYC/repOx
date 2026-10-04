# repOx VHS Terminal Demo Tape
# Run with: vhs assets/demo.tape
# Generates: assets/demo.gif

Output assets/demo.gif

Set Shell "zsh"
Set FontSize 14
Set Width 1200
Set Height 720
Set Theme "Catppuccin Mocha"
Set Padding 18
Set Framerate 30
Set PlaybackSpeed 1.0
Set LetterSpacing 0
Set LineHeight 1.2
Set CursorBlink false

# 1. Launch interactive prompt packer with Claude calibration
Type "repox -i -p claude"
Sleep 500ms
Enter
Sleep 1.5s

# 2. Navigate through the pre-order file tree
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms

# 3. Expand 'crates' directory
Type "l"
Sleep 400ms

# 4. Descend into 'crates/repox-core'
Type "j"
Sleep 200ms
Type "l"
Sleep 400ms

# 5. Inspect 'domain.rs' and observe preview updating in right pane
Type "j"
Sleep 700ms

# 6. Deselect a file and observe instant O(1) token gauge reduction
Type " "
Sleep 600ms
Type " "
Sleep 500ms

# 7. Select All / Invert demonstration
Type "a"
Sleep 800ms
Type "a"
Sleep 600ms

# 8. Trigger real-time fuzzy filter with nucleo-matcher
Type "/"
Sleep 300ms
Type "scanner"
Sleep 800ms
Backspace 7
Sleep 200ms
Type "tree"
Sleep 800ms
Escape
Sleep 400ms

# 9. Copy formatted context to clipboard and exit
Type "c"
Sleep 2.5s
