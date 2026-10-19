"""Optional site features shared by the build, preview and validation commands."""

import os
from pathlib import Path
import shutil
import tomllib


def tool_command(name, root):
    """Zola or Typst: the ZOLA / TYPST environment variable, then .tools/<name>/, then PATH.

    scripts/setup_tools.py installs the pinned versions into .tools/, so a copy
    there wins over a different version on PATH.
    """
    override = os.environ.get(name.upper())
    if override:
        return override
    bundled = Path(root) / '.tools' / name / (name + ('.exe' if os.name == 'nt' else ''))
    if bundled.is_file():
        return str(bundled)
    return shutil.which(name) or name


def read_config(site):
    return tomllib.loads((Path(site) / 'config.toml').read_text(encoding='utf-8-sig'))


def profile_enabled(config):
    """A new site can build without personal data or a Typst resume."""
    value = config.get('extra', {}).get('profile_enabled', False)
    if not isinstance(value, bool):
        raise ValueError('extra.profile_enabled must be true or false')
    return value


def require_profile_source(root):
    source = Path(root) / 'profile' / 'cv.typ'
    if not source.is_file():
        raise ValueError(
            'Profile is enabled but profile/cv.typ is missing. Add a profile source '
            'or set extra.profile_enabled = false in site/config.toml.')
    return source
