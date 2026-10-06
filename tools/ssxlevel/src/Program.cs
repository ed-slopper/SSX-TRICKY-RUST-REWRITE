using SSX_Library;
using SSXLibrary;
using SSXLibrary.FileHandlers.Models.Tricky;
using SSX_Library.EATextureLibrary;
using Newtonsoft.Json;
static class Cli {
    static int Main(string[] a) {
        if (a.Length < 3) { Console.WriteLine("ssxlevel unbig <file.big> <dir> | mkbig <dir> <file.big> | extract <file.big|level.map> <projectdir> | build <projectdir> <out.big>"); return 1; }
        switch (a[0]) {
            case "unbig": BIG.Extract(a[1], a[2]); break;
            case "mkbig": BIG.Create(BigType.C0FB, a[1], a[2], true); break;
            case "ssh2png": {
                // ssxlevel ssh2png <file.ssh> <outdir>   -> one PNG per image, named <file>_<image>.png
                var h = new OldShapeHandler();
                h.LoadShape(a[1]);
                Directory.CreateDirectory(a[2]);
                string stem = Path.GetFileNameWithoutExtension(a[1]);
                for (int i = 0; i < h.ShapeImages.Count; i++) {
                    h.BrightenImage(i);
                    h.ExtractSingleImage(Path.Combine(a[2], stem + (h.ShapeImages.Count > 1 ? "_" + i : "") + ".png"), i);
                }
                break; }
            case "model": {
                // ssxlevel model <out.json> <a.mpf> [b.mpf]   (body + head, or the board file) [lod index]
                var comb = new TrickyPS2ModelCombiner();
                int lod = 0;
                for (int i = 2; i < a.Length; i++) {
                    if (int.TryParse(a[i], out int l)) { lod = l; continue; }
                    var mpf = new TrickyPS2MPF();
                    mpf.load(a[i]);
                    comb.DectectModelType(mpf);
                }
                comb.StartReassignMesh(lod);
                var bones = new List<object>();
                foreach (var b in comb.bones) bones.Add(new { name = b.BoneName, parent = b.ParentBone, pos = new[] { b.Position.X, b.Position.Y, b.Position.Z }, rot = new[] { b.Radians.X, b.Radians.Y, b.Radians.Z } });
                var mats = new List<string>();
                foreach (var m in comb.materials) mats.Add(m.MainTexture);
                var meshes = new List<object>();
                foreach (var m in comb.reassignedMesh) {
                    var pos = new List<float>(); var nrm = new List<float>(); var uv = new List<float>(); var mat = new List<int>();
                    var bw = new List<int[]>();
                    foreach (var f in m.faces) {
                        foreach (var v in new[] { f.V1, f.V2, f.V3 }) { pos.Add(v.X); pos.Add(v.Y); pos.Add(v.Z); }
                        foreach (var v in new[] { f.Normal1, f.Normal2, f.Normal3 }) { nrm.Add(v.X); nrm.Add(v.Y); nrm.Add(v.Z); }
                        foreach (var v in new[] { f.UV1, f.UV2, f.UV3 }) { uv.Add(v.X); uv.Add(v.Y); }
                        foreach (var w in new[] { f.Weight1, f.Weight2, f.Weight3 }) {
                            var l = new List<int>();
                            if (w.boneWeights != null) foreach (var x in w.boneWeights) { l.Add(x.BoneID); l.Add(x.Weight); }
                            bw.Add(l.ToArray());
                        }
                        mat.Add(f.MaterialID);
                    }
                    meshes.Add(new { name = m.MeshName, shadow = m.ShadowModel, pos, nrm, uv, weights = bw, mat });
                }
                File.WriteAllText(a[1], JsonConvert.SerializeObject(new { bones, materials = mats, meshes }));
                Console.WriteLine($"{a[1]}: {comb.bones.Count} bones, {comb.materials.Count} materials, {comb.reassignedMesh.Count} meshes");
                break; }
            case "extract": {
                string map = a[1];
                if (map.ToLower().EndsWith(".big")) {
                    string tmp = Path.Combine(Path.GetTempPath(), "ssxlevel_" + Guid.NewGuid().ToString("N"));
                    BIG.Extract(a[1], tmp);
                    map = Directory.GetFiles(tmp, "*.map", SearchOption.AllDirectories)[0];
                }
                Directory.CreateDirectory(a[2]);
                var t = new TrickyLevelInterface();
                t.ExtractTrickyLevelFiles(map.Substring(0, map.Length - 4), a[2]);
                break; }
            case "build": {
                string tmp = Path.Combine(Path.GetTempPath(), "ssxlevel_" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(Path.Combine(tmp, "data", "models"));
                string name = Path.GetFileNameWithoutExtension(a[2]).ToLower();
                var t = new TrickyLevelInterface();
                t.LoadAndVerifyFiles(a[1]);
                t.BuildTrickyLevelFiles(a[1], Path.Combine(tmp, "data", "models", name + ".map"));
                BIG.Create(BigType.C0FB, tmp, a[2], true);
                if (a.Length > 3) Console.WriteLine("kept " + tmp); else Directory.Delete(tmp, true);
                break; }
        }
        return 0;
    }
}
