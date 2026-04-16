# GIF Recording Guide

This directory contains scripts for recording killer GIFs for the airis-workspace README.

## Prerequisites

### Option 1: VHS (Recommended)
```bash
brew install vhs
brew install ttyd  # Required dependency
```

### Option 2: asciinema + agg
```bash
brew install asciinema
cargo install --git https://github.com/asciinema/agg
```

### Option 3: QuickTime + ffmpeg
Record screen with QuickTime, then convert:
```bash
brew install ffmpeg gifsicle
```

---

## Recording Scripts

### 1. `airis init` Demo (01-init-demo.tape)

**Purpose**: Show the core value - generating all config files from manifest.toml

**VHS Recording**:
```bash
cd scripts/gif-recording
vhs 01-init-demo.tape
# Output: airis-init-demo.gif
```

**Manual Recording Steps** (if VHS fails):
```bash
# 1. Open terminal, start recording with asciinema or QuickTime
# 2. Run these commands:

cd /tmp && rm -rf airis-demo && mkdir airis-demo && cd airis-demo
ls -la
# Show: empty directory

airis init
# Shows: npm resolution, file generation

ls -la
# Shows: all generated files

tree -a -I 'node_modules|.git'
# Shows: complete structure
```

---

### 2. `airis doctor` Demo (TODO - requires implementation)

**Purpose**: Show LLM-era self-healing capability

**Script flow**:
1. Break package.json manually
2. Run `airis doctor`
3. Show automatic fix with git diff

---

### 3. Runtime Switch Demo (TODO)

**Purpose**: Show Docker-first with local GPU escape hatch

**Script flow**:
1. Show manifest.toml with `runtime = "docker"`
2. Run command in Docker
3. Change to `runtime = "local"`
4. Run same command locally with GPU

---

## Converting Videos to GIF

### From MP4 (QuickTime recording):
```bash
# High quality, optimized size
ffmpeg -i input.mp4 -vf "fps=10,scale=800:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse" -loop 0 output.gif

# Further optimization
gifsicle -O3 --colors 256 output.gif -o output-optimized.gif
```

### From asciinema:
```bash
# Record
asciinema rec demo.cast

# Convert to GIF
agg demo.cast demo.gif --theme mocha
```

---

## Best Practices

1. **Speed**: Keep GIFs under 10 seconds
2. **Size**: Target < 2MB for fast loading
3. **Font**: Use large font (18-20px) for readability
4. **Theme**: Dark theme (Catppuccin Mocha recommended)
5. **Padding**: Add padding around terminal

---

## Output Locations

After recording, place GIFs in:
```
assets/
├── airis-init-demo.gif
├── airis-doctor-demo.gif
└── airis-runtime-demo.gif
```

Then reference in README:
```markdown
![airis init demo](assets/airis-init-demo.gif)
```
