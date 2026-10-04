complete -c repox -s f -l format -d 'Format template: \'xml\' (Claude-optimized) or \'markdown\' / \'md\'' -r
complete -c repox -s o -l output -d 'Write formatted context to an output file instead of stdout' -r -F
complete -c repox -s p -l token-profile -d 'Tokenizer profile: \'claude\' (3.7/3.5), \'o1\' / \'o3\' / \'gpt4o\', \'deepseek\', \'gemini\', \'cl100k\'' -r
complete -c repox -s s -l max-file-size -d 'Skip files exceeding this size threshold (e.g. 500KB, 1.5MB)' -r
complete -c repox -s d -l max-depth -d 'Maximum directory nesting depth to traverse' -r
complete -c repox -s e -l exclude -d 'Glob pattern to exclude (can be specified multiple times)' -r
complete -c repox -s I -l include -d 'Glob pattern to include exclusively (can be specified multiple times)' -r
complete -c repox -s j -l threads -d 'Number of worker threads (defaults to logical CPU core count)' -r
complete -c repox -l completions -d 'Generate shell completions (bash, zsh, fish, powershell, elvish)' -r -f -a "bash\t''
elvish\t''
fish\t''
powershell\t''
zsh\t''"
complete -c repox -s c -l copy -d 'Copy output context directly to system clipboard'
complete -c repox -s t -l tokens -d 'Calculate total token count using multi-threaded tiktoken tokenizer'
complete -c repox -l no-gitignore -d 'Disable respecting .gitignore files'
complete -c repox -l no-repoxignore -d 'Disable respecting .repoxignore files'
complete -c repox -l include-hidden -d 'Include hidden files and folders'
complete -c repox -s i -l interactive -d 'Launch interactive terminal UI file picker (lazygit-style)'
complete -c repox -s q -l quiet -d 'Silence informational statistics on stderr'
complete -c repox -s v -l verbose -d 'Enable verbose debug logs'
complete -c repox -s h -l help -d 'Print help (see more with \'--help\')'
complete -c repox -s V -l version -d 'Print version'
