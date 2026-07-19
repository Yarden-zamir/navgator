1. The create picker is a modal opened from the main navigator UI with a `create` binding.
1. Key names in this spec describe the runtime default keymap and may be remapped by configuration.
2. The default navigator keymap binds `create` to Ctrl+N.
3. The UI shows the first resolved binding for create commands.
4. `navgator create` opens directly to the create picker.
5. `navgator create` uses the current directory as the selected path for `{path}` and `NAVGATOR_SELECTED_PATH`.
6. `navgator create <recipe>` opens directly to the matching create recipe.
7. `navgator create <recipe> <path>` opens directly to the matching create recipe with the given selected path.
8. Terminal recipe selectors match exact labels or label slugs, for example `New project` matches `new-project`.
9. The zsh wrapper provides `navgator-create`, which opens `navgator create` from `$PWD`.
10. The zsh wrapper provides `navgator-create-new-project`, which opens `navgator create new-project` from `$PWD`.
11. The create picker is unavailable while tag editing is active.
12. The create picker is unavailable while an action picker or progress overlay is active.
13. The create picker closes with Esc without running a recipe.
14. The create picker closes with Ctrl+C without running a recipe.
15. The create picker moves selection down with Down.
16. The create picker moves selection up with Up.
17. The create picker moves selection down with j.
18. The create picker moves selection up with k.
19. The create picker filters recipes when the user types a search query.
20. Backspace removes one character from the create search query.
21. Enter selects the highlighted create recipe.
22. A recipe with prompts opens a prompt form.
23. A recipe without prompts runs immediately.
24. Prompt forms show all prompts top down.
25. Prompt forms show prompt labels and prompt types.
26. Prompt forms reject empty required prompts.
27. Prompt forms run the recipe with Enter.
28. Prompt forms clear the active value with Ctrl+U.
29. Prompt forms return to the recipe picker with Esc.
30. Path prompts offer non-recursive filesystem suggestions.
31. Path prompt suggestions prefer directories before files.
32. Right focuses path prompt suggestions when suggestions are available.
33. Left returns focus from path prompt suggestions to the prompt fields.
34. Path prompt suggestions are accepted with Tab.
35. Path prompt suggestions move with j, k, Up, and Down while suggestions are focused.
36. Path prompt suggestions expand ~/ using HOME.
37. Up and Down move between prompt fields while fields are focused.
38. The cursor is placed on the active prompt value row while fields are focused.
39. Prompt defaults can reference earlier prompts with {prompt_name}.
40. Prompt defaults can reference the selected target with {path}.
41. Create recipe current_dir can reference prompts and {path}.
42. Create recipe success_path can reference prompts and {path}.
43. Shell recipes receive prompt values as NAVGATOR_CREATE_* environment variables.
44. Shell recipes receive the selected target as NAVGATOR_SELECTED_PATH when available.
45. Shell recipes run through the platform shell.
46. Shell recipe output may be shown in the progress overlay.
47. Shell recipe failure is shown in the progress overlay.
48. A failed create recipe keeps navgator open until the user dismisses the error.
49. A successful create recipe navigates to the expanded success_path automatically.
50. A successful create recipe must not navigate when success_path does not exist.
51. Built-in create recipes include New project.
52. Built-in create recipes include New branch + worktree.
53. The New project built-in prompts for project name and parent folder, then creates {parent}/{name}.
54. Config schema generation includes the create table.
55. Config schema generation includes create prompt types.
56. The implementation should avoid new dependencies.
57. Verification must include cargo fmt -- --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test, and cargo build --release.
