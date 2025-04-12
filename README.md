# Mailsweep

A command-line tool for cleaning up your Outlook/Microsoft 365 inbox using customizable rules.

## Features

- Authenticate with Microsoft Graph API using device code flow
- Define rules to process emails based on sender and/or subject patterns
- Actions include: archiving, deleting, or marking as read
- Interactive confirmation before applying changes
- Batch processing for better performance

## Installation

```
cargo install --path .
```

## Setup

1. The application uses a pre-registered Azure app with multitenant support and public client flow.

2. Authenticate with your Microsoft account:
```
mailsweep auth login
```

3. Rules and configuration files are stored according to the XDG Base Directory specification:
   - `$XDG_CONFIG_HOME/mailsweep/` if XDG_CONFIG_HOME is set (typically `~/.config/mailsweep/`)
   - Falls back to OS-appropriate locations if XDG_CONFIG_HOME is not set

4. Configure your rules:
```
mailsweep rules edit
```

Example rule file structure:
```yaml
# A rule that archives emails from newsletters
- name: Archive newsletters
  sender_contains:
    - newsletter
    - updates
  action: archive

# A rule that deletes promotional emails
- name: Delete promotions
  subject_contains:
    - discount
    - sale
    - offer
  action: delete

# A rule that requires both conditions to match
- name: Archive tech updates from company domain
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

## Usage

```
# Authentication commands
mailsweep auth login           # Authenticate with Microsoft Graph
mailsweep auth status          # Check authentication status
mailsweep auth logout          # Remove stored tokens

# Rules management
mailsweep rules show           # Show current rules
mailsweep rules edit           # Edit rules in your default editor
mailsweep rules path           # Get path to rules file
mailsweep rules check          # Validate rules for errors
mailsweep rules reset          # Reset rules to default (with confirmation)
mailsweep rules reset --force  # Force reset rules without confirmation

# Add a new rule via command line
mailsweep rules add --name "Archive newsletters" --action archive --sender newsletter --sender updates

# Process inbox
mailsweep clean                # Process inbox with interactive confirmation
mailsweep clean --max-messages 20  # Limit messages per page (pagination still applies)
mailsweep clean --yes          # Apply without confirmation prompt (for automation)
```

## Command Walkthrough

### Adding Rules

You can add rules in two ways:

1. Edit the rules file:
```
mailsweep rules edit
```

2. Use the command line:
```
# Add a rule to mark meeting invites as read
mailsweep rules add --name "Mark meeting invites as read" --action mark_read --subject "meeting invite" --subject calendar

# Add a rule to archive messages from a specific domain
mailsweep rules add --name "Archive company emails" --action archive --sender "@company.com"

# Add a rule requiring both sender and subject matches
mailsweep rules add --name "Delete spam" --action delete --sender "spam" --subject "limited offer"
```

### Cleaning Your Inbox

Running the clean command will:
1. Fetch messages from your inbox
2. Apply your rules to find matching messages
3. Show a table of matches with colored actions
4. Ask for confirmation before processing
5. Process messages in batches for better performance

```
mailsweep clean
```

## Editor Integration

The rules file includes a YAML header with a schema reference for enhanced editing features:

```yaml
# yaml-language-server: $schema=/path/to/your/config/dir/rules.schema.json
```

For VS Code users:
- Install the "YAML" extension by Red Hat
- Open your rules file (`mailsweep rules edit`)
- The editor will provide auto-completion, validation, and documentation

## License

MIT