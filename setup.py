"""generate setup file for installation"""
#!/usr/bin/env python
# coding: utf-8

import os
from setuptools import setup, find_packages

# Utility function to read README file to create the long_description.
def read(fname):
    return open(os.path.join(os.path.dirname(__file__), fname)).read()

setup(
    name="'''+self.project_path+'''",
    version="0.1.0",
    author="Conor Forde",
    author_email="me@conorforde",
    description=(""),
    license="LGPL3",
    keywords="martian data accumulator",
    url="",
    packages=find_packages(),
    long_description=read('README'),
    classifiers=[
        "Development Status :: 0.1.0 - Alpha",
        "Topic :: Utilities",
        "License :: LGPL3 License",
    ],
)
