#import <UIKit/UIKit.h>

// Rust sends a ScreenControls model as JSON (see workspace/controls.rs) and
// receives stable command ids back. Labels are display text only.
extern void datalith_mobile_command(const char *identifier);
static __weak UIViewController *surface;
static NSDictionary *controls;
static NSString *appliedControls;
static uint32_t backgroundRGBA;

static void send(NSString *identifier) { datalith_mobile_command(identifier.UTF8String); }

static UIAction *action(NSDictionary *entry) {
    NSString *identifier = entry[@"id"];
    UIAction *command = [UIAction actionWithTitle:entry[@"label"] image:[UIImage systemImageNamed:entry[@"symbol"]]
        identifier:nil handler:^(__kindof UIAction *sender) { send(identifier); }];
    if ([entry[@"destructive"] boolValue]) command.attributes = UIMenuElementAttributesDestructive;
    return command;
}

static UIBarButtonItem *vertical(UIBarButtonItem *button) {
#if __IPHONE_OS_VERSION_MAX_ALLOWED >= 270100
    if (@available(iOS 27.1, *)) button.axisBehavior = UIBarButtonItemAxisBehaviorVerticalPreferred;
#endif
    return button;
}

static UIBarButtonItem *item(NSDictionary *entry) {
    UIBarButtonItem *button = [[UIBarButtonItem alloc] initWithPrimaryAction:action(entry)];
    button.accessibilityLabel = entry[@"label"];
    return vertical(button);
}

static UIBarButtonItem *menu(NSString *label, NSString *symbol, NSArray<UIMenuElement *> *children) {
    UIBarButtonItem *button = [[UIBarButtonItem alloc] initWithImage:[UIImage systemImageNamed:symbol]
        menu:[UIMenu menuWithChildren:children]];
    button.accessibilityLabel = label;
    return vertical(button);
}

static NSArray<UIMenuElement *> *actions(NSArray *entries) {
    NSMutableArray *children = [NSMutableArray array];
    for (NSDictionary *entry in entries) [children addObject:action(entry)];
    return children;
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
        dispatch_async(dispatch_get_main_queue(), ^{ send(@"layout"); });
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
    NSDictionary *model = controls;
    id title = model[@"title"];
    controller.navigationItem.title = [title isKindOfClass:NSString.class] ? title : nil;
    BOOL back = [model[@"back"] boolValue];
    controller.navigationItem.leftBarButtonItem = back
        ? item(@{@"id": @"back", @"label": @"Back", @"symbol": @"chevron.backward"}) : nil;
    // rightBarButtonItems run from the trailing edge: More, then New, then Edit.
    NSMutableArray *trailing = [NSMutableArray array];
    NSArray *sections = model[@"menu"];
    if ([sections isKindOfClass:NSArray.class] && sections.count) {
        NSMutableArray *groups = [NSMutableArray array];
        for (NSArray *section in sections) {
            [groups addObject:[UIMenu menuWithTitle:@"" image:nil identifier:nil
                options:UIMenuOptionsDisplayInline children:actions(section)]];
        }
        [trailing addObject:menu(@"More actions", @"ellipsis.circle", groups)];
    }
    NSArray *create = model[@"create"];
    if ([create isKindOfClass:NSArray.class] && create.count) [trailing addObject:menu(@"New", @"plus", actions(create))];
    id primary = model[@"primary"];
    if ([primary isKindOfClass:NSDictionary.class]) [trailing addObject:item(primary)];
    controller.navigationItem.rightBarButtonItems = trailing;
    [navigation setNavigationBarHidden:!(back || trailing.count || controller.navigationItem.title) animated:NO];
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
        [[NSNotificationCenter defaultCenter] addObserverForName:UIContentSizeCategoryDidChangeNotification
            object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) { send(@"text-size"); }];
        applyControls();
    });
}

void datalith_set_controls_json(const char *json, uint32_t background) {
    NSString *text = [NSString stringWithUTF8String:json];
    if (background == backgroundRGBA && [text isEqualToString:appliedControls]) return;
    NSDictionary *model = [NSJSONSerialization JSONObjectWithData:[text dataUsingEncoding:NSUTF8StringEncoding]
        options:0 error:nil];
    if (![model isKindOfClass:NSDictionary.class]) return;
    appliedControls = text;
    backgroundRGBA = background;
    dispatch_async(dispatch_get_main_queue(), ^{
        controls = model;
        applyControls();
    });
}

UIViewController *datalith_presenter(void) { return surface; }

// The body text size the person chose in Settings › Display & Brightness ›
// Text Size (Dynamic Type): 17 pt by default.
double datalith_preferred_text_size(void) {
    return [UIFont preferredFontForTextStyle:UIFontTextStyleBody].pointSize;
}

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

// Commands on one object: an action sheet on iPhone, a popover anchored at
// the touch on iPad. Destructive commands keep UIKit's destructive style.
void datalith_actions(const char *title, const char *json, double x, double y) {
    NSString *heading = [NSString stringWithUTF8String:title];
    NSData *data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
    NSArray *entries = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!surface || surface.presentedViewController || ![entries isKindOfClass:NSArray.class]) {
            datalith_chose(-1);
            return;
        }
        UIAlertController *sheet = [UIAlertController alertControllerWithTitle:heading message:nil
            preferredStyle:UIAlertControllerStyleActionSheet];
        [entries enumerateObjectsUsingBlock:^(NSDictionary *entry, NSUInteger index, BOOL *stop) {
            UIAlertActionStyle style = [entry[@"destructive"] boolValue] ? UIAlertActionStyleDestructive : UIAlertActionStyleDefault;
            UIAlertAction *choice = [UIAlertAction actionWithTitle:entry[@"label"] style:style
                handler:^(UIAlertAction *a) { datalith_chose((int32_t)index); }];
            [sheet addAction:choice];
        }];
        [sheet addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel
            handler:^(UIAlertAction *a) { datalith_chose(-1); }]];
        UIPopoverPresentationController *popover = sheet.popoverPresentationController;
        if (popover) {
            popover.sourceView = surface.view;
            popover.sourceRect = CGRectMake(x, y, 1, 1);
        }
        [surface presentViewController:sheet animated:YES completion:nil];
    });
}
