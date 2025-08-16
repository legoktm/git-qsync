# Git-QSync Technical Specification

## Overview
A CLI tool for transferring git branches between Qubes VMs using git bundles and qvm-move.

## Command Interface

### Export Command: `git qsync export` (alias: `git qe`)
**Usage**: `git qe [branch]`

**Behavior**:
- Creates git bundle of specified branch (defaults to current branch)
- Generates bundle filename: `{project-name}_{branch-name}_{timestamp}.bundle`
- Executes `qvm-move bundle-file` (user will be prompted to select target VM)

### Import Command: `git qsync import` (alias: `git qi`)
**Usage**: `git qi [bundle-file]`

**Behavior**:
- Scans `~/QubesIncoming/{source-vm}/{project-name}/` for `.bundle` files
- Selects most recent bundle (by timestamp)
- Extracts branch name from bundle metadata
- Prompts for import confirmation with branch conflict warnings
- Fetches bundle and creates/updates local branch

## Configuration

### Git Config Integration (`~/.gitconfig`)
```ini
[qsync]
    source-vm = dev-vm    # Required for import
```

**Access via git config**:
```bash
# Set default source VM for imports
git config --global qsync.source-vm dev-vm
```

## Project Name Detection
Use directory basename:
```bash
PROJECT_NAME=$(basename "$(pwd)")
```

## Bundle Strategy
**Export**: Bundle from merge-base with default HEAD branch
```bash
# Find default branch and fork point
DEFAULT_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD | sed 's@^refs/remotes/origin/@@')
MERGE_BASE=$(git merge-base HEAD $DEFAULT_BRANCH)
git bundle create bundle.bundle $MERGE_BASE..HEAD
```

## Bundle Naming Convention
`{project-name}_{branch-name}_{iso-timestamp}.bundle`

Example: `myapp_feature-auth_2024-08-16T14-30-45.bundle`

## Conflict Resolution
When importing to existing branch, prompt user:
```
Branch 'feature-auth' already exists. Choose action:
[o] Overwrite existing branch (destructive)
[n] Import as new branch name
[c] Cancel import

Enter choice:
```

For new branch option, suggest: `import-{branch-name}`

## Bundle Verification
Git provides multiple verification levels:

### Basic Verification
```bash
git bundle verify bundle.bundle
# Returns: exit code 0 = valid, non-zero = invalid
# Checks: bundle format, object integrity, reference consistency
```

### Content Verification
```bash
git bundle list-heads bundle.bundle
# Shows: all refs and their commit SHAs
# Allows: preview of what will be imported before actual import
```

### Advanced Verification
```bash
# Verify bundle can be fetched into current repo
git bundle verify bundle.bundle HEAD
# Checks: bundle prerequisites are satisfied by current repo state
```

**Verification Strategy**:
1. Always run basic verification before import
2. Show bundle contents (branches, commit count) for user confirmation  
3. Verify prerequisites match current repo state
4. Proceed with import only after all checks pass

## Import Behavior
- **Repository validation**: Error if not in git repository
- **Bundle location**: `~/QubesIncoming/{source-vm}/{project-name}/`
- **Bundle selection**: Most recent `.bundle` file by timestamp
- **Verification errors**: Pass through git's native error messages
- **Missing bundles**: Emit clear error message

## Error Messages
```bash
# Outside git repo
Error: Not in a git repository

# No bundles found  
Error: No bundle files found in ~/QubesIncoming/dev-vm/myproject/

# Bundle verification (pass through git output)
Error: Bundle verification failed:
<git bundle verify output>
```
