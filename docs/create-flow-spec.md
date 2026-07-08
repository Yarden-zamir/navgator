1. The create picker is a modal opened from the main navigator UI with configured create bindings.
2. Create bindings default to Ctrl+N.
3. The UI shows only the first configured create binding.
4. The create picker is unavailable while tag editing is active.
5. The create picker is unavailable while an action picker or progress overlay is active.
6. The create picker closes with Esc without running a recipe.
7. The create picker closes with Ctrl+C without running a recipe.
8. The create picker moves selection down with Down.
9. The create picker moves selection up with Up.
10. The create picker moves selection down with j.
11. The create picker moves selection up with k.
12. The create picker filters recipes when the user types a search query.
13. Backspace removes one character from the create search query.
14. Enter selects the highlighted create recipe.
15. A recipe with prompts opens a prompt form.
16. A recipe without prompts runs immediately.
17. Prompt forms show all prompts top down.
18. Prompt forms show prompt labels and prompt types.
19. Prompt forms reject empty required prompts.
20. Prompt forms run the recipe with Enter.
21. Prompt forms clear the active value with Ctrl+U.
22. Prompt forms return to the recipe picker with Esc.
23. Path prompts offer non-recursive filesystem suggestions.
24. Path prompt suggestions prefer directories before files.
25. Right focuses path prompt suggestions when suggestions are available.
26. Left returns focus from path prompt suggestions to the prompt fields.
27. Path prompt suggestions are accepted with Tab.
28. Path prompt suggestions move with j, k, Up, and Down while suggestions are focused.
29. Path prompt suggestions expand ~/ using HOME.
30. Up and Down move between prompt fields while fields are focused.
31. The cursor is placed on the active prompt value row while fields are focused.
32. Prompt defaults can reference earlier prompts with {prompt_name}.
33. Prompt defaults can reference the selected target with {path}.
34. Create recipe current_dir can reference prompts and {path}.
35. Create recipe success_path can reference prompts and {path}.
36. Shell recipes receive prompt values as NAVGATOR_CREATE_* environment variables.
37. Shell recipes receive the selected target as NAVGATOR_SELECTED_PATH when available.
38. Shell recipes run through the platform shell.
39. Shell recipe output may be shown in the progress overlay.
40. Shell recipe failure is shown in the progress overlay.
41. A failed create recipe keeps navgator open until the user dismisses the error.
42. A successful create recipe navigates to the expanded success_path automatically.
43. A successful create recipe must not navigate when success_path does not exist.
44. Built-in create recipes include New project.
45. Built-in create recipes include New branch + worktree.
46. The New project built-in prompts for project name and parent folder, then creates {parent}/{name}.
47. Config schema generation includes the create table.
48. Config schema generation includes create prompt types.
49. The implementation should avoid new dependencies.
50. Verification must include cargo fmt -- --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test, and cargo build --release.
