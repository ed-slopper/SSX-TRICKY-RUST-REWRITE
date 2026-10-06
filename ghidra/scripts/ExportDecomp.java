// Decompiles every function and writes C to <outdir>/<range>.c plus functions.csv.
//@category SSX
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;
import java.util.*;
public class ExportDecomp extends GhidraScript {
    public void run() throws Exception {
        File out = new File(getScriptArgs()[0]); out.mkdirs();
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        PrintWriter csv = new PrintWriter(new File(out, "functions.csv"));
        csv.println("address,size,name,file");
        Map<String, PrintWriter> files = new HashMap<>();
        int n = 0, fail = 0;
        for (Function fn : currentProgram.getFunctionManager().getFunctions(true)) {
            if (monitor.isCancelled()) break;
            long a = fn.getEntryPoint().getOffset();
            String fname = String.format("%06x.c", a & ~0xFFFFL);
            PrintWriter w = files.get(fname);
            if (w == null) { w = new PrintWriter(new File(out, fname)); files.put(fname, w); }
            DecompileResults res = d.decompileFunction(fn, 60, monitor);
            csv.printf("%08x,%d,%s,%s%n", a, fn.getBody().getNumAddresses(), fn.getName(true), fname);
            w.printf("// ===== %08x  %s =====%n", a, fn.getName(true));
            if (res != null && res.decompileCompleted()) w.println(res.getDecompiledFunction().getC());
            else { w.println("// decompile failed"); fail++; }
            if (++n % 500 == 0) println("decompiled " + n);
        }
        for (PrintWriter w : files.values()) w.close();
        csv.close();
        println("done: " + n + " functions, " + fail + " failed");
    }
}
