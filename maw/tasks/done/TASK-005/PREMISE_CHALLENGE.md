# Premise challenge

## Counter-example tested

The Kenney Mini Characters archive may contain fewer than the assumed 32 animation clips, so recording 32 clips in the manifest could satisfy the checklist while misrepresenting the asset.

## Primary-source investigation

I downloaded the ZIP linked directly in the HTML of `https://kenney.nl/assets/mini-characters` to `scratch/kenney_mini-characters.zip`. The download command reported `bytes 2403059 sha256 9e1d48e6d7b8479ebbe84df71eb5bd8e1b3f0da546dea641890dccc8a02d0999`.

I ran `python maw/tasks/in_progress/TASK-005/scratch/audit_asset.py` against the ZIP's GLB JSON. Its output reported `character_glbs 12`, and for all 12 character GLBs: `animations 32 skin_joints [7, 7] unique_joints 7`. A direct read of `Models/GLB format/character-male-a.glb` reported two skins whose `joints` arrays are both `[1, 2, 3, 4, 5, 6, 7]`; the referenced nodes are `root`, `leg-left`, `leg-right`, `torso`, `arm-left`, `arm-right`, and `head`.

## Did it hold

The clip-count counter-example did not hold: each of the 12 character GLBs has 32 animations. The same raw asset exposed a different discrepancy in the task premise: its two skins each list seven joints, but both lists reference the same seven distinct nodes. The task's “14 bones” predicate would misstate this archive's GLB structure.

## Verdict

PREMISE SUSPECT — `python maw/tasks/in_progress/TASK-005/scratch/audit_asset.py` output: all 12 character GLBs have `skin_joints [7, 7] unique_joints 7`; direct GLB JSON read: both skins have `joints: [1, 2, 3, 4, 5, 6, 7]` ; smallest implied reframing: verify and record 12 characters, seven distinct joint nodes shared by two skins, and 32 clips per character for this SHA-256-pinned archive.
