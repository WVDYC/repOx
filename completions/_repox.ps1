
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'repox' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'repox'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'repox' {
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'Format template: ''xml'' (Claude-optimized) or ''markdown'' / ''md''')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Format template: ''xml'' (Claude-optimized) or ''markdown'' / ''md''')
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Write formatted context to an output file instead of stdout')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Write formatted context to an output file instead of stdout')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Tokenizer profile: ''fable'', ''luna'', ''gemini'', ''claude'', ''o1'', ''deepseek'', ''llama'', ''cl100k''')
            [CompletionResult]::new('--token-profile', '--token-profile', [CompletionResultType]::ParameterName, 'Tokenizer profile: ''fable'', ''luna'', ''gemini'', ''claude'', ''o1'', ''deepseek'', ''llama'', ''cl100k''')
            [CompletionResult]::new('-s', '-s', [CompletionResultType]::ParameterName, 'Skip files exceeding this size threshold (e.g. 500KB, 1.5MB)')
            [CompletionResult]::new('--max-file-size', '--max-file-size', [CompletionResultType]::ParameterName, 'Skip files exceeding this size threshold (e.g. 500KB, 1.5MB)')
            [CompletionResult]::new('-d', '-d', [CompletionResultType]::ParameterName, 'Maximum directory nesting depth to traverse')
            [CompletionResult]::new('--max-depth', '--max-depth', [CompletionResultType]::ParameterName, 'Maximum directory nesting depth to traverse')
            [CompletionResult]::new('-e', '-e', [CompletionResultType]::ParameterName, 'Glob pattern to exclude (can be specified multiple times)')
            [CompletionResult]::new('--exclude', '--exclude', [CompletionResultType]::ParameterName, 'Glob pattern to exclude (can be specified multiple times)')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'Glob pattern to include exclusively (can be specified multiple times)')
            [CompletionResult]::new('--include', '--include', [CompletionResultType]::ParameterName, 'Glob pattern to include exclusively (can be specified multiple times)')
            [CompletionResult]::new('-j', '-j', [CompletionResultType]::ParameterName, 'Number of worker threads (defaults to logical CPU core count)')
            [CompletionResult]::new('--threads', '--threads', [CompletionResultType]::ParameterName, 'Number of worker threads (defaults to logical CPU core count)')
            [CompletionResult]::new('--completions', '--completions', [CompletionResultType]::ParameterName, 'Generate shell completions (bash, zsh, fish, powershell, elvish)')
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'Copy output context directly to system clipboard')
            [CompletionResult]::new('--copy', '--copy', [CompletionResultType]::ParameterName, 'Copy output context directly to system clipboard')
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'Calculate total token count using multi-threaded tiktoken tokenizer')
            [CompletionResult]::new('--tokens', '--tokens', [CompletionResultType]::ParameterName, 'Calculate total token count using multi-threaded tiktoken tokenizer')
            [CompletionResult]::new('--no-gitignore', '--no-gitignore', [CompletionResultType]::ParameterName, 'Disable respecting .gitignore files')
            [CompletionResult]::new('--no-repoxignore', '--no-repoxignore', [CompletionResultType]::ParameterName, 'Disable respecting .repoxignore files')
            [CompletionResult]::new('--include-hidden', '--include-hidden', [CompletionResultType]::ParameterName, 'Include hidden files and folders')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'Launch interactive terminal UI file picker (lazygit-style)')
            [CompletionResult]::new('--interactive', '--interactive', [CompletionResultType]::ParameterName, 'Launch interactive terminal UI file picker (lazygit-style)')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Silence informational statistics on stderr')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Silence informational statistics on stderr')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Enable verbose debug logs')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Enable verbose debug logs')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
