# VHS Recording Script for "airis doctor" Demo
# https://github.com/charmbracelet/vhs
#
# Install: brew install vhs
# Record:  vhs 02-doctor-demo.tape
# Output:  airis-doctor-demo.gif

# Configuration
Output airis-doctor-demo.gif
Set FontSize 18
Set Width 900
Set Height 600
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 50ms

# Setup: Create a workspace first
Hide
Type "cd /tmp && rm -rf airis-doctor-demo && mkdir airis-doctor-demo && cd airis-doctor-demo && cp $HOME/.airis/fixtures/manifest.toml . && airis gen > /dev/null 2>&1"
Enter
Sleep 500ms
Show

# Show the scenario
Type "# LLM broke your package.json? Let's fix it!"
Enter
Sleep 1s

# Show original file (first few lines)
Type "head -5 package.json"
Enter
Sleep 1s

# Break the file (simulate LLM damage)
Type "# Simulating LLM damage..."
Enter
Sleep 500ms
Type 'echo '"'"'{"broken": true, "oops": "LLM deleted everything"}'"'"' > package.json'
Enter
Sleep 500ms

# Show broken file
Type "cat package.json"
Enter
Sleep 1s

# Run doctor to diagnose
Type "# Run airis doctor to diagnose"
Enter
Sleep 500ms
Type "airis doctor"
Enter
Sleep 2s

# Fix with --fix flag
Type "# Auto-repair with --fix"
Enter
Sleep 500ms
Type "airis doctor --fix"
Enter
Sleep 4s

# Verify fix
Type "# Verify: package.json is restored!"
Enter
Sleep 500ms
Type "head -10 package.json"
Enter
Sleep 2s

# End
Sleep 1s
