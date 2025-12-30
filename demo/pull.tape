Output demo/pull.gif

Set Shell "zsh"
Set FontSize 16
Set Width 1000
Set Height 600
Set Padding 20
Set Theme "Catppuccin Mocha"
Set WindowBar "Rings"
Set TypingSpeed 75ms

# Setup (assume some history exists)
Hide
Type "export PS1='> '"
Enter
Type "clear"
Enter
Show

# Demo: waylog pull
Type "tree .waylog/history"
Sleep 1s
Enter

Type "waylog pull"
Sleep 1s
Enter

# Wait for scanning and syncing output
Sleep 4s

# Show tree structure of synced files
Type "tree .waylog/history"
Sleep 200ms
Enter
Sleep 3s
