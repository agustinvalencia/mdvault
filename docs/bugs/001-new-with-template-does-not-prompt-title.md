# `mdv new` with template flag does not prompt for title

## Description 

When running `mdv new --template <some_template>` where `<some_template>` is not one of the core types, i.e. a custom user-defined type. The note title is not prompted.

## Repro steps

1. Consider the following markdown template

```md
---
type: project_resource
lua: project_resource.lua
---

# {{title}}


```

and its lua definition 

```lua
local M = {
	name = "project_resource",
	description = "Resource specific to one project",
	output = "Projects/{{project}}/resources/{{title|slugify}}.md",
	schema = {
		created_at = { type = "datetime", default = mdv.date("now") },
		updated_at = { type = "datetime", default = mdv.date("now") },
		project = {
			type = "string",
			prompt = "Project Folder",
		},
		title = { type = "string", core = true, prompt="Note Title" },
	},
}

M.on_create = function(note, ctx)
	return note
end

return M
```

2. Call `mdv new --template project_resource`

we are prompted for the project folder, but not for the note title 

3. Output file

Let's say we typed "my-project" when prompted for the project folder, 
the generated file will be saved in `Projects/my-project/resources/{{title|slugify}}.md`
where `{{title|slugify}}.md` is the actual file name. The note does not have any of the expected frontmatter fields either. 

## Considerations

It has been found that if called as 

```shell
mdv new --template project_resource "my-title"
```

Then the file is correctly created under `Projects/my-project/resources/my-title.md`, the frontmatter still is not there though
