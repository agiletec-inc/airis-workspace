# VHS Recording Script for "airis affected" Demo
# https://github.com/charmbracelet/vhs
#
# Install: brew install vhs
# Record:  vhs 03-affected-demo.tape
# Output:  airis-affected-demo.gif

# Configuration
Output airis-affected-demo.gif
Set FontSize 18
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 50ms

# Navigate to airis-workspace (a real monorepo)
Hide
Type "cd /Users/kazuki/github/airis-workspace"
Enter
Sleep 500ms
Show

# Show the command
Type "# Analyze which packages are affected by recent changes"
Enter
Sleep 1s

# Run airis affected
Type "airis affected"
Enter
Sleep 3s

# End
Sleep 1s
