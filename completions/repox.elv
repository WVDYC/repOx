
use builtin;
use str;

set edit:completion:arg-completer[repox] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'repox'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'repox'= {
            cand -f 'Format template: ''xml'' (Claude-optimized) or ''markdown'' / ''md'''
            cand --format 'Format template: ''xml'' (Claude-optimized) or ''markdown'' / ''md'''
            cand -o 'Write formatted context to an output file instead of stdout'
            cand --output 'Write formatted context to an output file instead of stdout'
            cand -p 'Tokenizer profile: ''fable'', ''luna'', ''claude'', ''o1'', ''deepseek'', ''gemini'', ''cl100k'''
            cand --token-profile 'Tokenizer profile: ''fable'', ''luna'', ''claude'', ''o1'', ''deepseek'', ''gemini'', ''cl100k'''
            cand -s 'Skip files exceeding this size threshold (e.g. 500KB, 1.5MB)'
            cand --max-file-size 'Skip files exceeding this size threshold (e.g. 500KB, 1.5MB)'
            cand -d 'Maximum directory nesting depth to traverse'
            cand --max-depth 'Maximum directory nesting depth to traverse'
            cand -e 'Glob pattern to exclude (can be specified multiple times)'
            cand --exclude 'Glob pattern to exclude (can be specified multiple times)'
            cand -I 'Glob pattern to include exclusively (can be specified multiple times)'
            cand --include 'Glob pattern to include exclusively (can be specified multiple times)'
            cand -j 'Number of worker threads (defaults to logical CPU core count)'
            cand --threads 'Number of worker threads (defaults to logical CPU core count)'
            cand --completions 'Generate shell completions (bash, zsh, fish, powershell, elvish)'
            cand -c 'Copy output context directly to system clipboard'
            cand --copy 'Copy output context directly to system clipboard'
            cand -t 'Calculate total token count using multi-threaded tiktoken tokenizer'
            cand --tokens 'Calculate total token count using multi-threaded tiktoken tokenizer'
            cand --no-gitignore 'Disable respecting .gitignore files'
            cand --no-repoxignore 'Disable respecting .repoxignore files'
            cand --include-hidden 'Include hidden files and folders'
            cand -i 'Launch interactive terminal UI file picker (lazygit-style)'
            cand --interactive 'Launch interactive terminal UI file picker (lazygit-style)'
            cand -q 'Silence informational statistics on stderr'
            cand --quiet 'Silence informational statistics on stderr'
            cand -v 'Enable verbose debug logs'
            cand --verbose 'Enable verbose debug logs'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
    ]
    $completions[$command]
}
