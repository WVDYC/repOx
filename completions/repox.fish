complete -c repox -s f -l format -d 'Format template: \'xml\' (Claude), \'markdown\' / \'md\', or \'tool-call\' (agent JSON)' -r
complete -c repox -s o -l output -d 'Write formatted context to an output file instead of stdout' -r -F
complete -c repox -s p -l token-profile -d 'Tokenizer profile: \'fable\', \'luna\', \'gemini\', \'claude\', \'o1\', \'deepseek\', \'llama\', \'cl100k\'' -r
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
complete -c repox -l budget -d 'Auto-compress (via outlines) and prune files to fit within token budget (e.g. 50k, 100k)' -r
complete -c repox -l video-prompt -d 'Optimize an AI video generation prompt (Grok, Kling, Sora, Veo, Wan 2.1) with character lock & 5s shots' -r
complete -c repox -s c -l copy -d 'Copy output context directly to system clipboard'
complete -c repox -s t -l tokens -d 'Calculate total token count using multi-threaded tiktoken tokenizer'
complete -c repox -l no-gitignore -d 'Disable respecting .gitignore files'
complete -c repox -l no-repoxignore -d 'Disable respecting .repoxignore files'
complete -c repox -l include-hidden -d 'Include hidden files and folders'
complete -c repox -s i -l interactive -d 'Launch interactive terminal UI file picker (lazygit-style)'
complete -c repox -s q -l quiet -d 'Silence informational statistics on stderr'
complete -c repox -s v -l verbose -d 'Enable verbose debug logs'
complete -c repox -l outline -d 'Extract architecture signatures and type outlines only (strips function bodies)'
complete -c repox -l summary-locks -d 'Summarize lockfiles into compact dependency manifests instead of skipping them'
complete -c repox -s m -l modified -d 'Only pack Git modified and untracked files'
complete -c repox -l staged -d 'Only pack Git staged files'
complete -c repox -s r -l redact-secrets -d 'Scan and redact inline secrets, API keys, and private keys with [REDACTED_SECRET]'
complete -c repox -s h -l help -d 'Print help (see more with \'--help\')'
complete -c repox -s V -l version -d 'Print version'
