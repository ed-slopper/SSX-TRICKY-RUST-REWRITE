// Applies class/vtable/virtual-function names recovered from GCC 2.95 RTTI (symbols.txt).
//@category SSX
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.*;
import java.io.*;
public class ApplySsxSymbols extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File f = args.length > 0 ? new File(args[0]) : askFile("symbols.txt", "Apply");
        BufferedReader r = new BufferedReader(new FileReader(f));
        String line; int n = 0;
        SymbolTable st = currentProgram.getSymbolTable();
        while ((line = r.readLine()) != null) {
            String[] p = line.trim().split(" ");
            if (p.length < 3) continue;
            if (p[0].equals("G") || p[0].equals("D")) {
                Address ga = toAddr(Long.parseLong(p[1], 16));
                try {
                    if (p[0].equals("G")) { Function gf = getFunctionAt(ga); if (gf != null) gf.setName(p[2], SourceType.USER_DEFINED); }
                    else st.createLabel(ga, p[2], SourceType.USER_DEFINED);
                    n++;
                } catch (Exception e) { println("skip " + line); }
                continue;
            }
            Address a = toAddr(Long.parseLong(p[1], 16));
            Namespace ns = st.getNamespace(p[2], currentProgram.getGlobalNamespace());
            if (ns == null) ns = st.createClass(currentProgram.getGlobalNamespace(), p[2], SourceType.USER_DEFINED);
            try {
                if (p[0].equals("F")) {
                    Function fn = getFunctionAt(a);
                    if (fn == null) { disassemble(a); fn = createFunction(a, null); }
                    if (fn == null) continue;
                    fn.setParentNamespace(ns);
                    fn.setName(p[3] + (p[3].startsWith("vf") ? "_" + p[1] : ""), SourceType.USER_DEFINED);
                } else {
                    st.createLabel(a, p[3], ns, SourceType.USER_DEFINED);
                }
                n++;
            } catch (Exception e) { println("skip " + line + ": " + e.getMessage()); }
        }
        r.close();
        println("applied " + n + " symbols");
    }
}
