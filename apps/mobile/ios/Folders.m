#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

extern UIViewController *datalith_presenter(void);
extern void datalith_folder_picked(const char *bookmark, const char *error);

@interface DatalithFolderPicker : NSObject <UIDocumentPickerDelegate>
@end
@implementation DatalithFolderPicker
- (void)documentPicker:(UIDocumentPickerViewController *)controller didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls {
    NSURL *url = urls.firstObject;
    if (![url startAccessingSecurityScopedResource]) {
        datalith_folder_picked(NULL, "Files did not grant access to the folder");
        return;
    }
    NSError *error = nil;
    NSData *bookmark = [url bookmarkDataWithOptions:NSURLBookmarkCreationMinimalBookmark
        includingResourceValuesForKeys:nil relativeToURL:nil error:&error];
    [url stopAccessingSecurityScopedResource];
    datalith_folder_picked([bookmark base64EncodedStringWithOptions:0].UTF8String, error.localizedDescription.UTF8String);
}
@end

void datalith_pick_folder(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        static DatalithFolderPicker *delegate;
        if (!delegate) delegate = [DatalithFolderPicker new];
        UIDocumentPickerViewController *picker = [[UIDocumentPickerViewController alloc]
            initForOpeningContentTypes:@[UTTypeFolder] asCopy:NO];
        picker.delegate = delegate;
        [datalith_presenter() presentViewController:picker animated:YES completion:nil];
    });
}

// The caller owns the scope and its retained NSURL until the vault is closed.
void *datalith_resolve_folder(const char *encoded, char **path, char **renewed) {
    NSData *data = [[NSData alloc] initWithBase64EncodedString:[NSString stringWithUTF8String:encoded] options:0];
    BOOL stale = NO;
    NSURL *url = [NSURL URLByResolvingBookmarkData:data options:NSURLBookmarkResolutionWithoutUI relativeToURL:nil bookmarkDataIsStale:&stale error:nil];
    if (!url.isFileURL || ![url startAccessingSecurityScopedResource]) return NULL;
    __block BOOL readable = NO;
    NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
    [coordinator coordinateReadingItemAtURL:url options:0 error:nil byAccessor:^(NSURL *folder) {
        readable = [[NSFileManager defaultManager] contentsOfDirectoryAtURL:folder includingPropertiesForKeys:nil options:0 error:nil] != nil;
    }];
    if (!readable) { [url stopAccessingSecurityScopedResource]; return NULL; }
    *path = strdup(url.fileSystemRepresentation);
    if (stale) {
        NSData *fresh = [url bookmarkDataWithOptions:NSURLBookmarkCreationMinimalBookmark includingResourceValuesForKeys:nil relativeToURL:nil error:nil];
        if (fresh) *renewed = strdup([fresh base64EncodedStringWithOptions:0].UTF8String);
    }
    return (__bridge_retained void *)url;
}
void datalith_release_folder(void *handle) {
    NSURL *url = (__bridge_transfer NSURL *)handle;
    [url stopAccessingSecurityScopedResource];
}
void datalith_free_folder_string(char *string) { free(string); }

// The callback runs synchronously on the coordinating thread. Both URLs may
// be adjusted by a file presenter, especially when moving an iCloud document.
typedef void (*FolderAccess)(void *, const char *, const char *);
char *datalith_coordinate(const char *first, const char *second, int operation, void *context, FolderAccess access) {
    @autoreleasepool {
        NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:first isDirectory:NO relativeToURL:nil];
        NSError *error = nil;
        if (second) {
            NSURL *destination = [NSURL fileURLWithFileSystemRepresentation:second isDirectory:NO relativeToURL:nil];
            [coordinator coordinateWritingItemAtURL:url options:NSFileCoordinatorWritingForMoving
                writingItemAtURL:destination options:0 error:&error byAccessor:^(NSURL *from, NSURL *to) {
                    access(context, from.fileSystemRepresentation, to.fileSystemRepresentation);
                }];
        } else if (operation == 0) {
            [coordinator coordinateReadingItemAtURL:url options:0 error:&error byAccessor:^(NSURL *item) {
                access(context, item.fileSystemRepresentation, NULL);
            }];
        } else {
            [coordinator coordinateWritingItemAtURL:url options:0 error:&error byAccessor:^(NSURL *item) {
                access(context, item.fileSystemRepresentation, NULL);
            }];
        }
        return error ? strdup(error.localizedDescription.UTF8String) : NULL;
    }
}
