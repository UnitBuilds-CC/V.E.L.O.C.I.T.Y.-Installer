# Create a reference MSI using Python msilib low-level API
import msilib, uuid, os, sys

outpath = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output\ref_python.msi"
if os.path.exists(outpath):
    os.remove(outpath)

# Create empty database
db = msilib.OpenDatabase(outpath, msilib.MSIDBOPEN_CREATEDIRECT)

# Add standard tables
msilib.add_tables(db, msilib.Directory)
msilib.add_tables(db, msilib.Feature)
msilib.add_tables(db, msilib.Binary)
msilib.add_tables(db, msilib.Control)
msilib.add_tables(db, msilib.Dialog)
msilib.add_tables(db, msilib.RadioButtonGroup)

# Create test files
test_dir = r"C:\Users\visse\OneDrive\Documentos\V.E.L.O.C.I.T.Y.-Installer-master\examples\sample-app\output\test_files"
os.makedirs(test_dir, exist_ok=True)
with open(os.path.join(test_dir, "hello.txt"), "w") as f:
    f.write("Hello from Python ref!")
with open(os.path.join(test_dir, "data.txt"), "w") as f:
    f.write("Test data from Python reference MSI")

# Add data
msilib.add_data(db, "Property", [
    ("ProductName", "Python Ref Test"),
    ("ProductVersion", "1.0.0"),
    ("Manufacturer", "Velocity Team"),
    ("ProductCode", str(uuid.uuid4())),
    ("UpgradeCode", str(uuid.uuid4())),
    ("ProductLanguage", "1033"),
])

msilib.add_data(db, "Directory", [
    ("TARGETDIR", None, "SourceDir"),
    ("LocalAppDataFolder", "TARGETDIR", "LocalAppData"),
    ("INSTALLDIR", "LocalAppDataFolder", "PythonRef:PythonRef"),
])

msilib.add_data(db, "Component", [
    ("comp_0", None, "INSTALLDIR", 0, None, "file_0"),
    ("comp_1", None, "INSTALLDIR", 0, None, "file_1"),
])

msilib.add_data(db, "File", [
    ("file_0", "comp_0", "hello.txt", 22, None, "", 0, 1),
    ("file_1", "comp_1", "data.txt", 37, None, "", 0, 2),
])

msilib.add_data(db, "Feature", [
    ("Complete", None, "Complete", "All files", 1, 1, "INSTALLDIR", 0),
])

msilib.add_data(db, "Feature", msilib.Feature)

msilib.add_data(db, "FeatureComponents", [
    ("Complete", "comp_0"),
    ("Complete", "comp_1"),
])

msilib.add_data(db, "Media", [
    (1, 2, "#PythonRef.cab", None, None, None),
])

msilib.add_data(db, "InstallExecuteSequence", [
    ("LaunchConditions", "NOT Installed", 100),
    ("CostInitialize", None, 800),
    ("FileCost", None, 900),
    ("CostFinalize", None, 1000),
    ("InstallValidate", None, 1400),
    ("InstallInitialize", None, 1500),
    ("ProcessComponents", None, 1600),
    ("InstallFiles", None, 4000),
    ("RegisterProduct", None, 6100),
    ("PublishFeatures", None, 6300),
    ("PublishProduct", None, 6400),
    ("InstallFinalize", None, 6600),
])

msilib.add_data(db, "InstallUISequence", [
    ("LaunchConditions", None, 100),
    ("CostInitialize", None, 800),
    ("CostFinalize", None, 1000),
    ("ExecuteAction", None, 1300),
])

# Add cabinet
cab = msilib.CAB("PythonRef")
cab.add_file(os.path.join(test_dir, "hello.txt"), "hello.txt", 22, False, "file_0")
cab.add_file(os.path.join(test_dir, "data.txt"), "data.txt", 37, False, "file_1")
cab.commit(db)

db.Commit()
print(f"Created: {outpath}")
print(f"Size: {os.path.getsize(outpath)} bytes")
