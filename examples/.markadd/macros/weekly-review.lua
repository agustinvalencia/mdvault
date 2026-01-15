-- Macro: Weekly review workflow
return {
    name = "weekly-review",
    description = "Set up weekly review documents and archive completed tasks",

    vars = {
        week_of = {
            prompt = "Week of (date)?",
            default = "{{today}}",
        },
        theme = "What's the theme for this week?",
    },

    steps = {
        {
            template = "weekly-summary",
            output = "weekly/{{week_of}}.md",
            with = {
                theme = "{{theme}}",
            },
        },
        {
            capture = "todo",
            with = {
                task = "Complete weekly review for {{week_of}}",
            },
        },
    },
}
