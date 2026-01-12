-- Capture: Add a task to the TODO section
return {
    name = "todo",
    description = "Add a task to the TODO section",

    vars = {
        task = "Task description?",
    },

    target = {
        file = "tasks.md",
        section = "TODO",
        position = "end",
    },

    content = "- [ ] {{task}} (added {{date}})",
}
