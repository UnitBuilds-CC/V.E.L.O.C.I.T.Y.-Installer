"""Create a minimal MSI using msilib and test with msiexec."""
import msilib
import os
import sys

os.system('taskkill /F /IM msiexec.exe > nul 2>&1')

# Method 1: Use OpenDatabase with MSIDBOPEN_CREATEDIRECT
path = 'python_test2.msi'
if os.path.exists(path):
    os.remove(path)

db = msilib.OpenDatabase(path, msilib.MSIDBOPEN_CREATEDIRECT)

# Create tables using MSI SQL
views = [
    "CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)",
    "CREATE TABLE `Directory` (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72), `DefaultDir` CHAR(255) NOT NULL PRIMARY KEY `Directory`)",
    "CREATE TABLE `InstallExecuteSequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)",
]

for sql in views:
    v = db.OpenView(sql)
    v.Execute(None)
    v.Close()

# Insert data using SQL INSERT
inserts = [
    "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductName', 'Python Test2')",
    "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductVersion', '1.0.0')",
    "INSERT INTO `Property` (`Property`, `Value`) VALUES ('Manufacturer', 'V')",
    "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductCode', '{AADDD4E4-3A12-4B7D-A4E0-5A2F8B3C6D90}')",
    "INSERT INTO `Property` (`Property`, `Value`) VALUES ('ProductLanguage', '1033')",
    "INSERT INTO `Directory` (`Directory`, `DefaultDir`) VALUES ('TARGETDIR', 'SourceDir')",
    "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('CostInitialize', 800)",
    "INSERT INTO `InstallExecuteSequence` (`Action`, `Sequence`) VALUES ('CostFinalize', 1000)",
]

for sql in inserts:
    v = db.OpenView(sql)
    v.Execute(None)
    v.Close()

# Set SummaryInformation properties
si = db.GetSummaryInformation(14)  # 14 = max properties we'll set
si.SetProperty(2, "Installation Database")  # Title
si.SetProperty(3, "Python Test2")  # Subject
si.SetProperty(4, "V")  # Author
si.SetProperty(7, "Intel;1033")  # Template
si.SetProperty(9, "{247F8300-3914-44B1-B83E-E1F741507FA3}")  # Rev Number
si.SetProperty(14, 200)  # Security
si.SetProperty(15, 2)  # Word Count
si.SetProperty(18, "Python MSI Library")  # Creating App

db.Commit()

print(f"Created: {path} ({os.path.getsize(path)} bytes)")

# Test with msiexec
ret = os.system(f'msiexec /i {path} /qn /norestart /l*v test2.log')
print(f"msiexec exit code: {ret}")

if ret != 0:
    # Check log
    if os.path.exists('test2.log'):
        with open('test2.log') as f:
            for line in f:
                if 'Error' in line or 'return value 3' in line:
                    print(f"  LOG: {line.strip()}")

# Also compare with python_ref.msi
print("\n=== Comparing SummaryInfo ===")
for fname in ['python_ref.msi', path]:
    if not os.path.exists(fname):
        continue
    db2 = msilib.OpenDatabase(fname, msilib.MSIDBOPEN_READONLY)
    si2 = db2.GetSummaryInformation(0)
    print(f"\n{fname}:")
    for pid in [1,2,3,4,7,9,14,15,18]:
        try:
            val = si2.GetProperty(pid)
            print(f"  PID {pid}: {val}")
        except:
            pass
