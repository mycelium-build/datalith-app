#import <UIKit/UIKit.h>

extern void datalith_mobile_action(int32_t action);
static __weak UIViewController *surface;
static BOOL canGoBack;
static int32_t editMode;
static BOOL canDelete;
static uint32_t backgroundRGBA;

static UIBarButtonItem *item(NSString *label, NSString *symbol, int32_t command) {
    UIAction *action = [UIAction actionWithTitle:label image:[UIImage systemImageNamed:symbol]
        identifier:nil handler:^(__kindof UIAction *sender) { datalith_mobile_action(command); }];
    UIBarButtonItem *button = [[UIBarButtonItem alloc] initWithPrimaryAction:action];
    button.accessibilityLabel = label;
#if __IPHONE_OS_VERSION_MAX_ALLOWED >= 270100
    if (@available(iOS 27.1, *)) button.axisBehavior = UIBarButtonItemAxisBehaviorVerticalPreferred;
#endif
    return button;
}

@interface DatalithNavigation : UINavigationController
@end

@implementation DatalithNavigation {
    CGRect previousBounds;
    UIEdgeInsets previousInsets;
}

- (void)viewDidLayoutSubviews {
    [super viewDidLayoutSubviews];
    UIView *view = self.topViewController.view;
    if (!CGRectEqualToRect(previousBounds, view.bounds) ||
        !UIEdgeInsetsEqualToEdgeInsets(previousInsets, view.safeAreaInsets)) {
        previousBounds = view.bounds;
        previousInsets = view.safeAreaInsets;
        dispatch_async(dispatch_get_main_queue(), ^{ datalith_mobile_action(3); });
    }
}
@end

static void applyControls(void) {
    UIViewController *controller = surface;
    if (!controller) return;
    UIColor *color = [UIColor colorWithRed:((backgroundRGBA >> 24) & 255) / 255.0
        green:((backgroundRGBA >> 16) & 255) / 255.0 blue:((backgroundRGBA >> 8) & 255) / 255.0 alpha:1];
    UINavigationController *navigation = controller.navigationController;
    navigation.view.backgroundColor = color;
    navigation.view.window.backgroundColor = color;
    navigation.navigationBar.backgroundColor = color;
    controller.navigationItem.leftBarButtonItem = canGoBack ? item(@"Back", @"chevron.backward", 0) : nil;
    NSMutableArray *actions = [NSMutableArray array];
    if (editMode) [actions addObject:editMode == 1 ? item(@"Edit", @"pencil", 1) : item(@"Read", @"book", 1)];
    if (canDelete) [actions addObject:item(@"Delete", @"trash", 2)];
    controller.navigationItem.rightBarButtonItems = actions;
    [controller.navigationController setNavigationBarHidden:!(canGoBack || actions.count) animated:NO];
}

// Keep the GPUI controller and text input view together. UIKit places only
// the navigation controls and adjusts the surface's safe area on display changes.
void datalith_install_controls(void *pointer) {
    UIView *view = (__bridge UIView *)pointer;
    dispatch_async(dispatch_get_main_queue(), ^{
        UIWindow *window = view.window;
        UIViewController *content = window.rootViewController;
        if (!content || surface) return;
        window.rootViewController = nil;
        content.edgesForExtendedLayout = UIRectEdgeNone;
        UINavigationController *navigation = [[DatalithNavigation alloc] initWithRootViewController:content];
        surface = content;
        window.rootViewController = navigation;
        applyControls();
    });
}

void datalith_set_controls(bool back, int32_t mode, bool writable, uint32_t background) {
    if (back == canGoBack && mode == editMode && writable == canDelete && background == backgroundRGBA) return;
    backgroundRGBA = background;
    canGoBack = back;
    editMode = mode;
    canDelete = writable;
    dispatch_async(dispatch_get_main_queue(), ^{ applyControls(); });
}
