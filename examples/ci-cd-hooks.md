# CI/CD Optimized Hooks Configuration

## Problem: Default Hooks Are Suboptimal for CI/CD

**Development (Interactive):**
- ✅ Background cache warming (don't block developers)
- ✅ Fail silently (don't break workflows)
- ✅ Quick cache checks (speed over cache hits)

**CI/CD (Automated):**  
- ❌ Background operations waste build time
- ❌ Silent failures mask infrastructure issues
- ❌ Cache misses cost more than cache warming

## Solution: CI/CD-Specific Hook Configuration

### Detect CI Environment

```toml
# .mise.toml - Auto-detect CI and adjust behavior
[hooks]
preinstall = '''
if [ -n "$CI" ] || [ -n "$GITHUB_ACTIONS" ] || [ -n "$GITLAB_CI" ]; then
  mise-s3-cache restore --ci-mode --all
else
  mise-s3-cache restore --hook-mode --all
fi
'''

postinstall = '''
if [ -n "$CI" ] || [ -n "$GITHUB_ACTIONS" ] || [ -n "$GITLAB_CI" ]; then
  mise-s3-cache store --all
else
  mise-s3-cache store --hook-mode --all
fi
'''

# In CI: foreground cache warming, in dev: background
enter = '''
if [ -n "$CI" ] || [ -n "$GITHUB_ACTIONS" ] || [ -n "$GITLAB_CI" ]; then
  mise-s3-cache warm --ci-mode --parallel=5
else
  mise-s3-cache warm --hook-mode --background
fi
'''
```

### Explicit CI Configuration

```toml
# .mise.ci.toml - Explicit CI configuration
[tools]
node = "20.11.0"
python = "3.11.7"

[hooks]
# CI hooks: fail-fast, foreground, aggressive caching
preinstall = "mise-s3-cache restore --ci-mode --all --parallel=8"
postinstall = "mise-s3-cache store --all --parallel=8"  
enter = "mise-s3-cache warm --ci-mode --parallel=8"
```

### Usage in CI Systems

#### GitHub Actions

```yaml
# .github/workflows/build.yml
name: Build
on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup mise with S3 cache
        run: |
          # Use CI-optimized configuration
          cp .mise.ci.toml .mise.toml
          
          # Trust the config
          mise trust
          
          # Install tools with aggressive caching
          mise install
        env:
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}  
          MISE_S3_CACHE_BUCKET: ${{ secrets.S3_CACHE_BUCKET }}
          MISE_S3_CACHE_REGION: us-east-1
          MISE_S3_CACHE_ENABLED: true
          CI: true
```

#### GitLab CI  

```yaml
# .gitlab-ci.yml
image: ubuntu:latest

variables:
  MISE_S3_CACHE_ENABLED: "true"
  MISE_S3_CACHE_BUCKET: "my-ci-cache-bucket"
  
before_script:
  - cp .mise.ci.toml .mise.toml
  - mise trust
  - mise install  # Will use CI-optimized hooks

build:
  script:
    - npm run build
    - npm test
```

## Performance Comparison

| Scenario | Default Hooks | CI-Optimized Hooks | Time Saved |
|----------|---------------|-------------------|------------|  
| **Fresh build** | 5min (download all) | 5min (same) | 0% |
| **Cached build** | 4min (background + download) | 30sec (restore from cache) | **87%** |
| **Partial cache** | 3min (some downloads) | 1min (restore + minimal downloads) | **67%** |

## CI Mode Benefits

### 1. **Aggressive Cache Restoration**
```bash
# Instead of quick check + download on miss:
mise-s3-cache restore --hook-mode --all  # Fast check, quick fail

# CI does thorough restoration:
mise-s3-cache restore --ci-mode --all --parallel=8  # Block until complete
```

### 2. **Fail-Fast on Cache Issues**
```bash
# Development: silently continue on cache errors
exit_code=0  # Always succeed in hook mode

# CI: fail build on cache configuration errors  
exit_code=$?  # Propagate errors in CI mode
```

### 3. **Parallel Cache Operations**
```bash
# Development: conservative parallelism (don't hog developer machine)
--parallel=3

# CI: aggressive parallelism (maximize CI machine resources)  
--parallel=8
```

### 4. **Verbose Logging**
```bash
# Development: quiet operation
--hook-mode  # Suppress output

# CI: detailed logging for debugging
--ci-mode    # Show detailed progress
```

## Environment Variable Controls

```bash
# Force CI behavior regardless of environment detection
export MISE_S3_CACHE_CI_MODE=true

# Override parallelism for CI
export MISE_S3_CACHE_CI_PARALLEL=10

# CI-specific bucket (separate from dev cache)
export MISE_S3_CACHE_CI_BUCKET=company-ci-cache-bucket
```

## Migration Strategy

1. **Phase 1**: Keep existing hooks, add CI detection
2. **Phase 2**: Test CI-specific configuration in staging  
3. **Phase 3**: Roll out CI optimizations to production
4. **Phase 4**: Optimize based on CI build time metrics

The key insight: **CI/CD environments should prioritize cache hit rates over responsiveness**, since the cost of waiting for cache restoration is much lower than downloading and building tools from scratch.