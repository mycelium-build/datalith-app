#import <UIKit/UIKit.h>

extern void datalith_mobile_action(int32_t action);
static __weak UIViewController *surface;
static BOOL canGoBack;
static BOOL canGoForward;
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
    if (canGoForward) [actions addObject:item(@"Forward", @"chevron.forward", 6)];
    if (canGoBack || editMode || canDelete) {
        UIAction *library = [UIAction actionWithTitle:@"Library" image:[UIImage systemImageNamed:@"books.vertical"]
            identifier:nil handler:^(__kindof UIAction *sender) { datalith_mobile_action(4); }];
        UIAction *documents = [UIAction actionWithTitle:@"Open documents" image:[UIImage systemImageNamed:@"square.stack"]
            identifier:nil handler:^(__kindof UIAction *sender) { datalith_mobile_action(7); }];
        UIAction *settings = [UIAction actionWithTitle:@"Settings" image:[UIImage systemImageNamed:@"gearshape"]
            identifier:nil handler:^(__kindof UIAction *sender) { datalith_mobile_action(8); }];
        UIAction *remove = [UIAction actionWithTitle:@"Delete" image:[UIImage systemImageNamed:@"trash"]
            identifier:nil handler:^(__kindof UIAction *sender) { datalith_mobile_action(2); }];
        remove.attributes = UIMenuElementAttributesDestructive;
        UIBarButtonItem *more = [[UIBarButtonItem alloc] initWithImage:[UIImage systemImageNamed:@"ellipsis.circle"]
            menu:[UIMenu menuWithChildren:canDelete ? @[library, documents, settings, remove] : @[library, documents, settings]]];
        more.accessibilityLabel = @"More actions";
        [actions addObject:more];
    }
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

void datalith_set_controls(bool back, bool forward, int32_t mode, bool writable, uint32_t background) {
    if (forward == canGoForward && back == canGoBack && mode == editMode && writable == canDelete && background == backgroundRGBA) return;
    backgroundRGBA = background;
    canGoBack = back;
    canGoForward = forward;
    editMode = mode;
    canDelete = writable;
    dispatch_async(dispatch_get_main_queue(), ^{ applyControls(); });
}

UIViewController *datalith_presenter(void) { return surface; }

extern void datalith_chose(int32_t index);
@interface DatalithChoices : UITableViewController <UIAdaptivePresentationControllerDelegate>
@property(nonatomic, copy) NSArray<NSString *> *options;
@property(nonatomic) NSInteger selected;
@end
@implementation DatalithChoices
- (NSInteger)tableView:(UITableView *)table numberOfRowsInSection:(NSInteger)section { return self.options.count; }
- (UITableViewCell *)tableView:(UITableView *)table cellForRowAtIndexPath:(NSIndexPath *)index {
    UITableViewCell *cell = [table dequeueReusableCellWithIdentifier:@"choice"];
    if (!cell) cell = [[UITableViewCell alloc] initWithStyle:UITableViewCellStyleDefault reuseIdentifier:@"choice"];
    cell.textLabel.text = self.options[index.row];
    cell.accessoryType = index.row == self.selected ? UITableViewCellAccessoryCheckmark : UITableViewCellAccessoryNone;
    return cell;
}
- (void)finish:(NSInteger)result {
    [self dismissViewControllerAnimated:YES completion:^{ datalith_chose((int32_t)result); }];
}
- (void)cancel { [self finish:-1]; }
- (void)presentationControllerDidDismiss:(UIPresentationController *)presentationController { datalith_chose(-1); }
- (void)tableView:(UITableView *)table didSelectRowAtIndexPath:(NSIndexPath *)index { [self finish:index.row]; }
@end

void datalith_choose(const char *title, const char *json, int32_t selected) {
    NSString *heading = [NSString stringWithUTF8String:title];
    NSData *data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
    NSArray *options = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!surface || surface.presentedViewController) { datalith_chose(-1); return; }
        DatalithChoices *choices = [[DatalithChoices alloc] initWithStyle:UITableViewStyleInsetGrouped];
        choices.options = options;
        choices.selected = selected;
        choices.title = heading;
        choices.navigationItem.leftBarButtonItem = [[UIBarButtonItem alloc] initWithBarButtonSystemItem:UIBarButtonSystemItemCancel target:choices action:@selector(cancel)];
        UINavigationController *sheet = [[UINavigationController alloc] initWithRootViewController:choices];
        sheet.presentationController.delegate = choices;
        [surface presentViewController:sheet animated:YES completion:nil];
    });
}
