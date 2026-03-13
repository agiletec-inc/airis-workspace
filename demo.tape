# airis Demo GIF
# Run: vhs demo.tape

Output assets/airis-demo.gif

Set FontSize 16
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set TypingSpeed 50ms
Set Padding 20

# 1. Show manifest.toml (single source of truth)
Type 'cat manifest.toml | head -30'
Enter
Sleep 3s

# 2. Generate all workspace files from manifest.toml
Type "airis gen"
Enter
Sleep 3s

# 3. Guard demo - pnpm is blocked on host
Type "pnpm install"
Enter
Sleep 3s
