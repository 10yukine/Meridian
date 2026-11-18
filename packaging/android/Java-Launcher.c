#include <dlfcn.h>
#include <stdio.h>

typedef int (*JliLaunch)(int, char **, int, const char **, int, const char **,
                         const char *, const char *, const char *, const char *,
                         unsigned char, unsigned char, unsigned char, int);

int main(int argc, char **argv) {
    void *library = dlopen("libjli.so", RTLD_NOW | RTLD_GLOBAL);
    if (!library) {
        fprintf(stderr, "%s\n", dlerror());
        return 1;
    }
    JliLaunch launch = (JliLaunch)dlsym(library, "JLI_Launch");
    if (!launch) {
        fprintf(stderr, "%s\n", dlerror());
        return 1;
    }
    return launch(argc, argv, 0, NULL, 0, NULL, "25", "25", "java", "java",
                  0, 0, 0, 0);
}
