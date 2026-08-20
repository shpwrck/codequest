# Conditional compatibility libraries -- sourced by AppRun before the app starts.
#
# libstdc++ and libfreetype are carried for hosts that are too OLD: RHEL 9 has
# libstdc++ 3.4.29 and FreeType 2.10.4, while this build needs GLIBCXX_3.4.30
# and FreeType 2.11's COLRv1 API (FT_Get_Paint et al).
#
# They must never shadow a NEWER host's copies. A 3.4.30 libstdc++ placed in
# front of Fedora 43's Mesa (which needs 3.4.32+) makes EGL initialisation fail
# with EGL_BAD_PARAMETER, and the app opens a window that renders nothing. That
# was found by bisecting the bundle against eglinfo, not by inspection -- so
# each library is injected only when the host's own copy is genuinely older.

cqa_host_lib() {
    _p=$(ldconfig -p 2>/dev/null | grep -F "$1" | grep -F 'x86-64' | sed -n 's/.* => //p' | head -1)
    if [ -z "$_p" ]; then
        for _d in /lib64 /usr/lib64 /usr/lib/x86_64-linux-gnu /lib/x86_64-linux-gnu; do
            [ -e "$_d/$1" ] && { _p="$_d/$1"; break; }
        done
    fi
    printf '%s' "$_p"
}

# Version strings live in the .so itself, so grep is enough -- no extra tooling
# on the user's machine, which is the whole point of shipping an AppImage.
cqa_lib=$(cqa_host_lib libstdc++.so.6)
if [ -z "$cqa_lib" ] || ! grep -qa 'GLIBCXX_3\.4\.30' "$cqa_lib" 2>/dev/null; then
    export LD_LIBRARY_PATH="$this_dir/usr/optional/libstdc++${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

cqa_lib=$(cqa_host_lib libfreetype.so.6)
if [ -z "$cqa_lib" ] || ! grep -qa 'FT_Get_Color_Glyph_Paint' "$cqa_lib" 2>/dev/null; then
    export LD_LIBRARY_PATH="$this_dir/usr/optional/libfreetype${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

unset cqa_lib
true
