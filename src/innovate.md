% Innovative --help messages

Last week I heard about how [Google Gemini][0] was offering a very generous
free tier on their API. So yesterday I wrote a small CLI application to
interface with Gemini. It took about 30 minutes to get something reasonable
working. I was satisfied - but something was missing.

Fortunately, I woke up with an idea to innovate on a crusty old staple of CLI
applications: the help message. Most _boring_ CLI applications would do
something like this:

```go
func main() {
        // Handle help message
        for _, arg := range os.Args {
                if arg == "-h" || arg == "--help" {
                        fmt.Fprintf(os.Stderr, "llm [-][context]..\n")
                        return
                }
        }

        [...]
```

Fast and reliable; who would want that? Bleh!

Try this on for size instead:

```go
//go:embed main.go
var src string

func help() {
        // Boring, non-innovate help message
        if innovate := os.Getenv("INNOVATE"); innovate == "" {
                fmt.Fprintf(os.Stderr, "llm [-][context]..\n")
                return
        }

        // Now we're talking - it's time to innovate
        prompt := `
                Given the following go program that compiles to a binary titled
                'llm', print out an appropriate --help message.  Your response
                should be human readable text suitable to immediately print to
                terminal.  Do not use any code blocks or backticks.  Only show
                short form help text which at minimum explains the positional
                parameters.  Do not show anything like a man page.
        `
        parts := []genai.Part{
                genai.Text(prompt),
                genai.Text(src),
        }
        if err := ask(parts); err != nil {
                log.Fatalf("Failed to innovate: %v", err)
        }
}

[...]
```

Now you can innovate while you're getting help:

```
$ INNOVATE=1 llm -h
Usage: llm [-][context]..

Queries gemini to provide conversation continuation.  If no parameters are
provided, input is read from stdin.  Parameters beginning with '-' are
considered prompts and passed directly to stdin.
```

Try out `llm` [here][1]!

[0]: https://ai.google.dev/
[1]: https://github.com/danobi/llm
