import sys
from pathlib import Path

SCRIPTS_PATH = Path(__file__).resolve().parents[2] / ".github/workflows/scripts"
if str(SCRIPTS_PATH) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_PATH))
