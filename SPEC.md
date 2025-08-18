# Git-QSync Technical Specification

## Overview
A CLI tool for transferring git branches between Qubes VMs using git bundles and qvm-move.

## Command Interface

### Setup: `git qsync init`
**Usage**: `git qsync init`

**Behavior**:
- Sets up global git aliases for `git qe` and `git qi` shortcuts
- Configures `git qe` to call `git qsync export`
- Configures `git qi` to call `git qsync import`
- One-time setup required before using shortcuts

### Export Command: `git qsync export` (shortcut: `git qe`)
**Usage**: `git qe [branch]` or `git qsync export [branch]`

**Behavior**:
- Creates git bundle of specified branch (defaults to current branch)
- Generates bundle filename: `{project-name}_{branch-name}_{timestamp}.bundle`
- Creates directory structure `$tmpdir/git-qsync/{project-name}/`
- Executes `qvm-move $tmpdir/git-qsync` (user will be prompted to select target VM)

### Import Command: `git qsync import` (shortcut: `git qi`)
**Usage**: `git qi [bundle-file]` or `git qsync import [bundle-file]`

**Behavior**:
- Scans `~/QubesIncoming/{source-vm}/git-qsync/{project-name}/` for `.bundle` files
- Selects most recent bundle (by timestamp)
- Extracts branch name from bundle metadata
- Prompts for import confirmation with branch conflict warnings
- Fetches bundle and creates/updates local branch

## Configuration

### Git Config Integration (`~/.gitconfig`)
```ini
[qsync]
    source-vm = dev-vm    # Required for import
    
[alias]
    qe = !git-qsync export   # Set up by 'git qsync init'
    qi = !git-qsync import   # Set up by 'git qsync init'
```

**Setup and configuration**:
```bash
# One-time setup to enable shortcuts
git qsync init

# Set default source VM for imports
git config --global qsync.source-vm dev-vm
```

## Project Name Detection
Use directory basename:
```bash
PROJECT_NAME=$(basename "$(pwd)")
```

## Bundle Strategy
**Export**: 
- For feature branches: Bundle from merge-base with default HEAD branch
- For default branch (main/master): Bundle entire branch history

```bash
# For feature branches - find default branch and fork point
DEFAULT_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD | sed 's@^refs/remotes/origin/@@')
MERGE_BASE=$(git merge-base HEAD $DEFAULT_BRANCH)
git bundle create bundle.bundle $MERGE_BASE..HEAD

# For default branch - export entire history
git bundle create bundle.bundle HEAD
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
- **Bundle location**: `~/QubesIncoming/{source-vm}/git-qsync/{project-name}/`
- **Bundle selection**: Most recent `.bundle` file by timestamp
- **Verification errors**: Pass through git's native error messages
- **Missing bundles**: Emit clear error message
- **Branch overwriting**: When overwriting existing branches, safely handle currently checked out branches
- **Post-import switching**: Automatically switch to imported branch after successful import

### Advanced Branch Handling
When importing a bundle to overwrite an existing branch:

1. **Current branch detection**: Uses gix library for efficient branch state detection
2. **Safe deletion**: If the target branch is currently checked out:
   - Attempts to switch to `main` or `master` if available
   - Falls back to creating a temporary branch if needed
   - Deletes the existing branch safely
3. **Import execution**: Fetches bundle content to the target branch
4. **Automatic switching**: Switches to the newly imported branch upon completion

This ensures that import operations are safe and user-friendly, preventing git repository corruption from deleting currently active branches.

## Performance and Implementation Details

### Git Integration
- **gix library**: Uses the high-performance `gix` Rust library for git operations where possible
  - Branch existence checking: Native rust implementation via `gix::refs::find()`
  - Current branch detection: Efficient HEAD analysis via `gix::Repository::head()`
  - Repository validation: Fast git repository detection
- **Git commands**: Falls back to git CLI for complex operations requiring worktree management:
  - Branch switching (`git checkout`)
  - Branch deletion (`git branch -D`)  
  - Bundle operations (`git bundle`, `git fetch`)

This hybrid approach provides the best of both worlds: fast, native operations for simple tasks while maintaining compatibility and reliability for complex git workflows.

## Error Messages
```bash
# Outside git repo
Error: Not in a git repository

# No bundles found  
Error: No bundle files found in ~/QubesIncoming/dev-vm/git-qsync/myproject/

# Bundle verification (pass through git output)
Error: Bundle verification failed:
<git bundle verify output>
```
