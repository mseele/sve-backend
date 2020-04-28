package de.sve.backend.manager;

import java.text.NumberFormat;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.List;
import java.util.Locale;
import java.util.stream.Collectors;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.gson.Gson;

import de.sve.backend.Utils;
import de.sve.backend.mail.Mail;
import de.sve.backend.mail.MailAccount;
import de.sve.backend.model.events.BookingResponse;
import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventBooking;
import de.sve.backend.model.events.EventCounter;
import de.sve.backend.model.events.EventType;
import de.sve.backend.model.news.NewsType;
import de.sve.backend.model.news.Subscription;
import de.sve.backend.sheets.EventsSheetController;
import de.sve.backend.store.DataStore;

public class EventsManager {

	private static final Logger LOG = LoggerFactory.getLogger(EventsManager.class);

	private static final Locale DE = Locale.GERMANY;

	private static DateTimeFormatter DATE_FORMAT = DateTimeFormatter.ofPattern("E, dd. MMM yyyy, HH:mm", DE); //$NON-NLS-1$

	private static DateTimeFormatter PAYDAY_FORMAT = DateTimeFormatter.ofPattern("dd. MMMM", DE); //$NON-NLS-1$

	private static NumberFormat PRICE_FORMAT = NumberFormat.getCurrencyInstance(DE);

	public static List<Event> events(boolean beta) throws Exception {
		return events(beta, null);
	}

	public static List<Event> events(Boolean beta, EventType type) throws Exception {
		return DataStore.events()
				.stream()
		 		.filter(e -> e.visible() && (type == null || type == e.type()) && (beta == null || beta.booleanValue() == e.beta()))
		 		.sorted((event1, event2) -> {
					 if (event1.isBookedUp() == event2.isBookedUp()) {
						 return event1.sortIndex().compareTo(event2.sortIndex());
					 } else if (event1.isBookedUp()) {
						 return 1;
					 }
					 return -1;
				 })
		 		.collect(Collectors.toList());
	}

	public static List<EventCounter> eventsCounter() throws Exception {
		return events(null, null).stream()
					   			 .map(EventCounter::create)
					   			 .collect(Collectors.toList());
	}

	public static BookingResponse booking(EventBooking booking) {
		try {
			Event event = DataStore.event(booking.eventId());
			if (event.maxSubscribers() == -1 || event.subscribers() < event.maxSubscribers()) {
				return successfullBooking(booking, event.bookEvent(), true);
			} else if (event.waitingList() < event.maxWaitingList()) {
				return successfullBooking(booking, event.bookEvent(), false);
			}
			LOG.error("Booking failed because Event (" + event.id() + ") was overbooked."); //$NON-NLS-1$ //$NON-NLS-2$
			String message = "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal."; //$NON-NLS-1$
			return BookingResponse.failure(message);
		} catch (Throwable t) {
			LOG.error("Booking failed", t); //$NON-NLS-1$
			String message = "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal."; //$NON-NLS-1$
			return BookingResponse.failure(message);
		}
	}

	private static BookingResponse successfullBooking(EventBooking booking, Event event, boolean isBooking) throws Throwable {
		String result = EventsSheetController.saveBooking(booking, event);
		if (booking.subscribeUpdates()) {
			NewsType type = event.type() == EventType.Events ? NewsType.Events : NewsType.Fitness;
			Subscription subscription = Subscription.create(booking.email(), type);
			NewsManager.subscribe(subscription, false);
		}
		sendMail(booking, event, isBooking);
		DataStore.save(event);
		LOG.info("Booking of Event (" + event.id() + ") was successfull: " + result); //$NON-NLS-1$ //$NON-NLS-2$
		String message;
		if (isBooking) {
			message = "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail."; //$NON-NLS-1$
		} else {
			message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden."; //$NON-NLS-1$
		}
		return BookingResponse.success(message, eventsCounter());
	}

	private static void sendMail(EventBooking booking, Event event, boolean isBooking) throws Throwable {
		try {
			EventType type = event.type();
			MailAccount account = MailAccount.of(type);
			Mail.Builder builder = Mail.via(account);
			String subjectPrefix;
			switch (type) {
				case Fitness:
					subjectPrefix = "[Fitness"; //$NON-NLS-1$
					break;
				case Events:
					subjectPrefix = "[Events"; //$NON-NLS-1$
					break;
				default:
					throw new IllegalArgumentException("Type '" + type + "' is not supported");  //$NON-NLS-1$//$NON-NLS-2$
			}
			subjectPrefix += "@SVE]"; //$NON-NLS-1$
			String template;
			if (isBooking) {
				builder.subject(subjectPrefix + " Bestätigung Buchung"); //$NON-NLS-1$
				template = event.bookingTemplate();
			} else {
				builder.subject(subjectPrefix + " Bestätigung Warteliste"); //$NON-NLS-1$
				template = event.waitingTemplate();
			}

			template = template.replace("${firstname}", booking.firstName().trim()); //$NON-NLS-1$
			template = template.replace("${lastname}", booking.lastName().trim()); //$NON-NLS-1$
			template = template.replace("${name}", event.name().trim()); //$NON-NLS-1$
			template = template.replace("${location}", event.location()); //$NON-NLS-1$
			StringBuilder dates = new StringBuilder();
			LocalDateTime payday = null;
			for (LocalDateTime date : event.dates()) {
				if (dates.length() > 0) {
					dates.append("\n"); //$NON-NLS-1$
				}
				dates.append("- "); //$NON-NLS-1$
				dates.append(DATE_FORMAT.format(date));
				dates.append(" Uhr"); //$NON-NLS-1$
				if (payday == null) {
					payday = date.minusDays(14);
					if (payday.isBefore(LocalDateTime.now())) {
						payday = LocalDateTime.now();
					}
				}
			}
			template = template.replace("${dates}", dates.toString()); //$NON-NLS-1$
			if (payday != null) {
				template = template.replace("${payday}", PAYDAY_FORMAT.format(payday)); //$NON-NLS-1$
			}
			Double cost = booking.cost(event);
			String price = PRICE_FORMAT.format(cost);
			template = template.replace("${price}", price); //$NON-NLS-1$
			StringBuilder content = new StringBuilder(template);
			if (booking.subscribeUpdates()) {
				String typeName = type == EventType.Events ? "Events" : "Kursangebote"; //$NON-NLS-1$ //$NON-NLS-2$
				content.append("\n\nPS: Ab sofort erhältst Du automatisch eine E-Mail, sobald neue " + typeName + " online sind.\n" + //$NON-NLS-1$ //$NON-NLS-2$
							   "\n" + //$NON-NLS-1$
							   "Solltest Du an unserem E-Mail-Service kein Interesse mehr haben, kannst Du dich ganz einfach von diesem Angebot abmelden. \n" + //$NON-NLS-1$
							"Klicke hierzu einfach auf folgenden Link:\n"); //$NON-NLS-1$
				content.append(Utils.urlBuilder()
					    .addParameter("unsubscribe", type.toString()) //$NON-NLS-1$
					    .addParameter("email", booking.email()) //$NON-NLS-1$
					    .toString());
			}
			builder.content(content.toString())
				   .to(booking.email())
				   .bcc(account.email())
				   .send();

			LOG.info("Booking email was send successfully"); //$NON-NLS-1$
		} catch (Throwable e) {
			Gson gson = Utils.gson();
			String message = "Error while sending mail for booking (" + gson.toJson(booking) + ") of event (" + gson.toJson(event) + ")."; //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
			LOG.error(message, e);
			throw e;
		}
	}

	public static Event update(Event event) throws Exception {
		DataStore.save(event);
		return DataStore.event(event.id());
	}

	public static void delete(Event event) throws Exception {
		DataStore.delete(event);
	}

}
