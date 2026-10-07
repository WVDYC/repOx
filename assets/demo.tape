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

# 1. Quick CLI showcase: architectural outline & automatic token budget fitter
Type "repox --outline -t -p claude -c"
Sleep 400ms
Enter
Sleep 1.2s

Type "repox --budget 20k -c"
Sleep 400ms
Enter
Sleep 1.2s

# 2. Launch interactive prompt packer with Claude Fable 5.1 (1M) calibration
Type "repox -i -p fable"
Sleep 500ms
Enter
Sleep 1.5s

# 3. Navigate through the pre-order file tree & preview lexical syntax highlighting
Type "j"
Sleep 250ms
Type "j"
Sleep 250ms

# 4. Expand 'crates' directory and descend into 'crates/repox-core/src'
Type "l"
Sleep 400ms
Type "j"
Sleep 200ms
Type "l"
Sleep 400ms
Type "j"
Sleep 700ms

# 5. Toggle selection with Space and observe instant O(1) token gauge updates
Type " "
Sleep 600ms
Type " "
Sleep 500ms

# 6. Trigger real-time fuzzy search filter with '/'
Type "/"
Sleep 300ms
Type "scanner"
Sleep 800ms
Escape
Sleep 400ms

# 7. Copy formatted context to clipboard with 'c' and exit
Type "c"
Sleep 2.5s
