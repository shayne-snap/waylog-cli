Output demo/run.gif

Set Shell "zsh"
Set FontSize 16
Set Width 1000
Set Height 600
Set Padding 20
Set Theme "Catppuccin Mocha"
Set WindowBar "Rings"
Set TypingSpeed 75ms

# Setup (clean environment)
Hide
Type "rm -rf .waylog"
Enter
Type "export PS1='> '"
Enter
Type "clear"
Enter
Show

# Demo: waylog run
Type "waylog run claude"
Sleep 500ms
Enter
Sleep 1s

# Initializing...
Sleep 2s

# Simulate interaction inside Claude
Type "Hi, how do I read a file in Rust?"
Sleep 500ms
Enter

# Simulate thinking/response time
Sleep 14s

# Exit the session
Type "/exit"
Sleep 500ms
Enter

# Wait for sync message to appear
Sleep 2s

# Verify output
Type "ls -1 .waylog/history/"
Sleep 500ms
Enter
Sleep 1s

# Show content
Type "head -n 15 .waylog/history/*.md"
Enter
Sleep 5s
