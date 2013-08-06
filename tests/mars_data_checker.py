"""Runs pylint on mars_data.py with desired attributes"""
import subprocess, os

tested_file = "mars_data"
tested_file_python = "../"+tested_file+".py"
subprocess.call("pyreverse --project="+tested_file+"UML --filter-mode=ALL "+tested_file_python, 
    shell=True)

subprocess.call("dot -Tpng classes_"+tested_file+"UML.dot -o "+tested_file+"UML.png", shell=True)
os.remove("classes_"+tested_file+"UML.dot")
os.rename(tested_file+"UML.png", "../uml/"+tested_file+"_uml.png")

subprocess.call("pylint --const-rgx='[a-z_][a-z0-9_]{2,30}$'" \
    " --disable=RP0401 --disable=RP0001 --disable=RP0002 " \
    "--disable=RP0101 --disable=RP0701 --disable=RP0801 "+tested_file_python, shell=True)

subprocess.call("coverage run "+tested_file_python, shell=True)
subprocess.call("coverage report", shell=True)
