# Mailsweep

A command-line tool for cleaning up your Outlook/Microsoft 365 inbox using customizable rules.

## Features

- Authenticate with Microsoft Graph API using device code flow
- Define rules to process emails based on sender and/or subject patterns
- Actions include: archiving, deleting, or marking as read
- Interactive confirmation before applying changes
- Batch processing for better performance
- Cross-platform: works on Windows, macOS, and Linux

## Installation

### Download pre-built binaries

The easiest way to install Mailsweep is to download the pre-built binaries from the [latest release](https://github.com/mortenscheel/mailsweep/releases/latest) page.

Choose the appropriate binary for your platform:
- Windows: `mailsweep-windows-amd64.exe`
- macOS (Intel): `mailsweep-macos-amd64`
- macOS (Apple Silicon): `mailsweep-macos-arm64`
- Linux: `mailsweep-linux-amd64`

After downloading, you may need to make the binary executable on macOS/Linux:
```bash
chmod +x mailsweep-macos-arm64
```

### Build from source

If you prefer to build from source, you'll need Rust installed:

```bash
# Clone the repository
git clone https://github.com/mortenscheel/mailsweep.git
cd mailsweep

# Build and install
cargo install --path .
```

## Quick Start

1. **Authenticate with your Microsoft account**:
   ```bash
   mailsweep auth login
   ```
   Follow the instructions to complete the authentication flow.

2. **Create your first rule**:
   ```bash
   # Add a rule to archive newsletters
   mailsweep rules add --name "Archive newsletters" --action archive --sender "newsletter" --sender "updates"
   ```

3. **Process your inbox**:
   ```bash
   mailsweep clean
   ```
   Review the matches and confirm to apply the actions.

## Configuration

Mailsweep stores rules and tokens in your user configuration directory:
- Linux/macOS: `~/.config/mailsweep/`
- Windows: `%APPDATA%\mailsweep\`

The rules file is stored as YAML in `rules.yaml`.

### Example Rules File

```yaml
# Archive emails from newsletters
- name: Archive newsletters
  sender_contains:
    - newsletter
    - updates
  action: archive

# Delete promotional emails
- name: Delete promotions
  subject_contains:
    - discount
    - sale
    - offer
  action: delete

# Archive tech updates from company domain (requires both to match)
- name: Archive tech updates
  sender_contains:
    - @company.com
  subject_contains:
    - tech update
    - technology news
  action: archive
```

## Rules Behavior

- Each rule must have at least one pattern for sender or subject (or both)
- Patterns are matched case-insensitively using a "contains" strategy
- If both `sender_contains` and `subject_contains` are specified, a message must match at least one pattern from each for the rule to apply
- The first matching rule determines the action to take on a message
- Rules are processed in the order they appear in the file

## Command Reference

### Authentication Commands

```bash
# Start authentication flow with Microsoft Graph
mailsweep auth login

# Check if you're authenticated and view account info
mailsweep auth status

# Log out and remove stored tokens
mailsweep auth logout
```

### Shell Completions

Mailsweep supports generating shell completions for various shells:

```bash
# Generate completions for your shell
mailsweep completions <shell>

# Available shells: bash, zsh, fish, posh, elvish

# Examples:
# Generate bash completions
mailsweep completions bash > ~/.bash_completion.d/mailsweep

# Generate zsh completions
mailsweep completions zsh > ~/.zsh_completions/_mailsweep

# Generate fish completions
mailsweep completions fish > ~/.config/fish/completions/mailsweep.fish

# Generate PowerShell completions
mailsweep completions posh > mailsweep.ps1

# Optional: output to file directly using --output flag
mailsweep completions bash --output ~/.bash_completion.d/mailsweep
```

For shell-specific installation instructions:

- **Bash**: Place in `~/.bash_completion.d/` and ensure it's sourced in your `.bashrc`
- **Zsh**: Place in a directory in your `$fpath` (like `~/.zsh_completions/`) and ensure completions are initialized
- **Fish**: Place in `~/.config/fish/completions/`
- **PowerShell**: Source the file in your profile

### Rules Management

```bash
# Display all current rules
mailsweep rules show

# Open rules in your default editor
mailsweep rules edit

# Get the path to your rules file
mailsweep rules path

# Validate your rules for errors
mailsweep rules check

# Reset rules to default (with confirmation)
mailsweep rules reset

# Reset without confirmation prompt
mailsweep rules reset --force
```

### Adding Rules via Command Line

The `rules add` command lets you create rules without editing the YAML file directly:

```bash
# Basic usage
mailsweep rules add --name "Rule name" --action <action> [--sender <pattern>...] [--subject <pattern>...]

# Available actions
# - archive
# - delete
# - mark_read (or markread)

# Examples:
# Archive newsletters
mailsweep rules add --name "Archive newsletters" --action archive --sender "newsletter" --sender "updates"

# Delete promotions
mailsweep rules add --name "Delete promotions" --action delete --subject "discount" --subject "sale"

# Mark as read (with both sender and subject patterns)
mailsweep rules add --name "Mark company announcements" --action mark_read --sender "@company.com" --subject "announcement"
```

You can specify multiple `--sender` and `--subject` patterns. Each parameter adds one pattern to the list.

### Processing Inbox

```bash
# Process inbox with interactive confirmation
mailsweep clean

# Process inbox with a specific number of messages per page
# (Pagination still applies to fetch all messages)
mailsweep clean --max-messages 50

# Process inbox without confirmation prompt (for automation)
mailsweep clean --yes
```

## Typical Workflow

1. **Setup** (first time only):
   ```bash
   mailsweep auth login
   ```

2. **Create Rules** (either using the editor or command line):
   ```bash
   # Edit rules in your editor
   mailsweep rules edit
   
   # Or add rules via command line
   mailsweep rules add --name "Archive newsletters" --action archive --sender "newsletter"
   ```

3. **Validate Rules**:
   ```bash
   mailsweep rules check
   ```

4. **Process Inbox**:
   ```bash
   mailsweep clean
   ```

5. **Review and Confirm**:
   - Mailsweep will display matching messages with their actions
   - Confirm to proceed or cancel to make changes

## Editor Integration

The rules file includes a YAML header with a schema reference for enhanced editing features:

```yaml
# yaml-language-server: $schema=/path/to/your/config/dir/rules.schema.json
```

For VS Code users:
- Install the "YAML" extension by Red Hat
- Open your rules file (`mailsweep rules edit`)
- The editor will provide auto-completion, validation, and documentation

## Troubleshooting

### Authentication Issues
- If you encounter authentication errors, try `mailsweep auth logout` followed by `mailsweep auth login`
- Check your account permissions for mail access

### No Messages Processed
- Verify your rules with `mailsweep rules check`
- Ensure that your rules match the expected email patterns
- Check that you have messages matching your rule criteria

### Command Not Found
- Ensure that the binary is in your PATH
- For Windows users, you may need to use `mailsweep.exe` instead of `mailsweep`

## License

MIT