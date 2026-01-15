-- Macro: Set up daily note with inbox items
return {
    name = "daily-setup",
    description = "Create today's daily note and add initial items",

    vars = {
        focus = {
            prompt = "What's your focus for today?",
            default = "",
        },
    },

    steps = {
        {
            type = "template",
            template = "daily",
            output = "daily/{{date}}.md",
            with = {
                focus = "{{focus}}",
            },
        },
        {
            type = "capture",
            capture = "inbox",
            with = {
                text = "Review focus: {{focus}}",
            },
        },
    },

    on_error = "abort",
}
