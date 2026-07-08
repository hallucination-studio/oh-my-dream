import json

from assistant.skills import load_enabled_skills


def test_loads_enabled_declarative_skills(tmp_path):
    skills_root = tmp_path / "skills"
    skill_root = skills_root / "portrait-helper"
    skill_root.mkdir(parents=True)
    (skill_root / "skill.json").write_text(
        json.dumps(
            {
                "name": "portrait-helper",
                "version": "1.0.0",
                "description": "Portrait helper",
                "capabilities": ["workflow.add_node"],
                "requires": {},
            }
        ),
        encoding="utf-8",
    )
    (skill_root / "prompt.md").write_text("Help build portrait workflows.\n", encoding="utf-8")

    skills = load_enabled_skills(tmp_path, ["portrait-helper"], developer_mode=False)

    assert [skill.name for skill in skills] == ["portrait-helper"]
    assert skills[0].prompt == "Help build portrait workflows.\n"
    assert skills[0].capabilities == ["workflow.add_node"]


def test_skips_code_skills_when_developer_mode_is_disabled(tmp_path):
    skills_root = tmp_path / "skills"
    skill_root = skills_root / "code-skill"
    skill_root.mkdir(parents=True)
    (skill_root / "skill.json").write_text(
        json.dumps(
            {
                "name": "code-skill",
                "version": "1.0.0",
                "description": "Code skill",
                "capabilities": [],
                "requires": {},
            }
        ),
        encoding="utf-8",
    )
    (skill_root / "prompt.md").write_text("Run code.\n", encoding="utf-8")
    (skill_root / "graph.py").write_text("print('unsafe')\n", encoding="utf-8")

    assert load_enabled_skills(tmp_path, ["code-skill"], developer_mode=False) == []
