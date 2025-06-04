BEGIN {
    packageInsert = "\"./snippets/*\", "

    jsAppend = "import { checkEncryptedTunnel, testWASM, persistenceCheck, initEncryptedTunnel } from \"./layer8_interceptor_rs.js\";\n" \
               "export default {\n" \
               "    checkEncryptedTunnel,\n" \
               "    testWASM,\n" \
               "    persistenceCheck,\n" \
               "    initEncryptedTunnel,\n" \
               "};"
}

FILENAME == "./pkg/package.json" {
    if (NR == 17) { # Line 17 is an arbitrary line; adjust as needed or rather find a cleaner solution to all this 💀
        print $0, packageInsert
    } else {
        print $0
    }
    next
}

FILENAME == "./pkg/layer8_interceptor_rs.js" {
    print $0
}

END {
    if (FILENAME == "./pkg/layer8_interceptor_rs.js") {
        print jsAppend
    }
}
