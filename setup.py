"""generate setup file for installation"""

import os
from setuptools import setup, find_packages

# Utility function to read the README file.
# Used for the long_description. It's nice, because now 1) we have a top level
# README file and 2) it's easier to type in the README file than to put a raw
# string in below ...
def read(fname):
    return open(os.path.join(os.path.dirname(__file__), fname)).read()

setup(
    name = "'''+self.project_path+'''",
    version = "0.0.1",
    author = "Conor Forde",
    author_email = "me@conorforde",
    description = (""),
    license = "LGPL3",
    keywords = "martian data accumulator",
    url = "",
    packages=find_packages(),
    long_description=read('README'),
    classifiers=[
        "Development Status :: 1 - Alpha",
        "Topic :: Utilities",
        "License :: LGPL3 License",
    ],
)
