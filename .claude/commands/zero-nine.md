Run the local Zero_Nine orchestration engine for the current project through one continuous host-native loop.

Use this same command on every turn:

`zero-nine run --host claude-code --project . --goal "$ARGUMENTS"`

On the first turn, `$ARGUMENTS` is the user goal. If Brainstorming is not yet Ready, Zero_Nine will treat the next invocation as the answer to the latest clarification question instead of starting a new run. Keep invoking the same command with only the latest answer until Zero_Nine reports that Brainstorming is Ready, the OpenSpec bundle is bound, and execution can continue.
