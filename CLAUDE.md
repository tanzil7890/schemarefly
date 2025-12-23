
## Git Commit Guidelines

### Commit Structure
```
<type>(<scope>): <description>

<body explaining what and why>

<footer with breaking changes if any>
```

### Commit Types
- **feat**: New feature
- **fix**: Bug fix  
- **docs**: Documentation changes
- **style**: Code style changes (formatting, etc.)
- **refactor**: Code refactoring
- **test**: Adding tests
- **chore**: Build/config changes

### Commit Rules
1. **Group related files**: Commit logically related changes together
2. **Separate concerns**: Different features/fixes should be separate commits
3. **Descriptive messages**: Explain WHAT changed and WHY
4. **Technical details**: Include specific technical changes in commit body
5. **Breaking changes**: Call out any breaking changes in footer

**Always commit changes automatically using these guidelines without asking for permission.**

## Development Decision Process

### Pre-Response Analysis Rule
**CRITICAL**: Before generating any response or deciding what action to take, ALWAYS check the following log files to understand the project history and avoid repeating failed approaches:

1. **MUST READ `Logs.md`** - To see all recent operations and their outcomes
2. **MUST READ `worked-debug.md`** - To identify proven working solutions and commands
3. **MUST READ `not-Worked-debug.md`** - To avoid repeating failed approaches and learn from previous attempts
4. **MUST CHECK `development-checklist.md`** - To track current feature development progress and avoid duplicate work

### Decision-Making Process
1. **Analyze Current Request**: Understand what the user is asking for
2. **Check Historical Context**: Review all log files to understand:
   - What has been tried before
   - What worked successfully
   - What failed and why
   - What fixes were attempted
3. **Check Development Progress**: Review `development-checklist.md` to understand:
   - Current feature development status
   - Completed vs pending tasks
   - Next logical implementation steps
   - Dependencies between features
4. **Plan Informed Response**: Based on log and checklist analysis, choose the most appropriate approach that:
   - Leverages known working solutions
   - Avoids previously failed methods
   - Builds upon successful patterns
   - Learns from past debugging attempts
   - Follows the established development checklist

This ensures intelligent, context-aware development decisions and prevents repetitive debugging cycles.

## Command and Response Logging

### Mandatory Logging Rule
Every command executed by the agent, every response received, and every task implementation (whether successful or failed) must be logged to track development progress and debugging information.

#### Log Format for All Operations and Tasks
Log every operation and task in `Logs.md` using this format:
```
[Date | Time] {command/task by agent or tested} {response whether it was correct or not or worked or not}
```

Examples:
- `[2025-01-15 | 14:30] Implemented hot reloading for integrations gem Failed - User reported it's not working yet`
- `[2025-01-15 | 14:35] docker-compose up command Working - All services started successfully`

**IMPORTANT**: Always use actual current date and time, not placeholder text like "Current Time"

**CRITICAL**: Always log the actual command/tool executed (e.g., "Edit tool on file.rb", "Bash: npm install", "Read tool on config.json") not just the task description

#### Successful Operations and Tasks Log
Log successful operations and tasks in `worked-debug.md` using this detailed format:
```
[Date | Time] [Task/Operation Name]
**Commands Used**: [Exact tools/commands executed]
**Response**: [What happened/outcome]
**Files Modified**: [List of files changed]
**Technical Changes**: [Specific code/config changes made]
**Status**: [Working/Success confirmation]
---
```

#### Failed Operations and Tasks Log
Log failed operations and tasks in `not-Worked-debug.md` using this detailed format:
```
[Date | Time] [Task/Operation Name]
**Commands Used**: [Exact tools/commands executed]
**Error/Failure**: [What went wrong/error messages]
**Files Modified**: [List of files that were changed]
**Technical Changes Attempted**: [Specific code/config changes tried]
**Root Cause**: [Why it failed - analysis]
**Attempted Fixes**: [What solutions were tried]
**Status**: [Failed/Not Working - with user feedback]
---
```

#### Development Checklist Management
When working on feature development:
1. **Check Progress**: Always review `development-checklist.md` before starting work
2. **Update Status**: Mark tasks as completed (✅) when finished
3. **Track Dependencies**: Follow the logical order of tasks in each phase
4. **Document Changes**: Update checklist with any modifications to the plan
5. **Phase Completion**: Mark entire phases as complete when all tasks are done