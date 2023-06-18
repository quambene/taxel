from typing import Any
import sys
from ebilanz import ebilanz


def main() -> None:
    file_path = '../output/elster_ebilanz.xml'
    file = open(file_path, 'w')

    rootObject: Any = ebilanz.parse('../templates/elster_ebilanz.xml')
    rootObject.export(file, 0)


if __name__ == "__main__":
    main()
