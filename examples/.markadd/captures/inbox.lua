-- Capture: Add a quick note to today's inbox
return {
    name = "inbox",
    description = "Add a quick note to today's inbox",

    vars = {
        text = "What to capture?",
    },

    target = {
        file = "daily/{{date}}.md",
        section = "Inbox",
        position = "begin",
    },

    content = "- [ ] {{text}}",
}
