# Release Packaging Instructions

## Building a Release for All Platforms

### Automatic (recommended)

```bash
git tag v0.1.0
git push origin v0.1.0
```

This triggers the GitHub Actions workflow which:
1. Builds `arcb` for Windows (x64), Linux (x64), and macOS (x64)
2. Creates a GitHub Release with all artifacts attached

Download from: GitHub -> Releases -> v0.1.0
