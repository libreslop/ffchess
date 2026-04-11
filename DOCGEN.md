You will generate documentation for this project.

First, read all files in the project. Then write documentation
in the folder docs/

It should contain a subfolder called docs/config/ in it,
for each config file type there should be a corresponding
markdown file describing everything about the config file,
including:
- the meaning of the file, what does it represent in the project
- where it should be placed/its naming/path
- a list of all possible attributes, description of the attribute
  all the possible values it can take, is it optional, etc
- some example config files with description (jsonc comments would be useful here)

It should contain a subfolder called docs/logic/ in it
for each chunk of logic flow, describe how everything works in detail
write in chapters where each chapter covers one topic in the program
each chapter gets its own file
you may draw mermaid diagrams to explain things

then, write a docs/README.md file giving a brief overview, and links to the files
in docs/ where appropriate

finally, write a master /README.md file giving a introduction of the project
