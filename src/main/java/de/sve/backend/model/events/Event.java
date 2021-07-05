package de.sve.backend.model.events;

import static java.util.Objects.requireNonNullElse;

import java.time.LocalDateTime;
import java.util.List;
import java.util.Optional;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Event {

	public static Event create(String id, String sheetId, Long gid, EventType type, String name, Long sortIndex, Boolean visible, Boolean beta, String shortDescription, String description,
			String image, Boolean light, List<LocalDateTime> dates, String customDate, Long durationInMinutes, Long maxSubscribers, Long subscribers, Double costMember, Double costNonMember,
			Long waitingList, Long maxWaitingList, String location, String bookingTemplate, String waitingTemplate, Boolean externalOperator) {
		return new AutoValue_Event(id, sheetId, gid, type, name, sortIndex, visible, beta, shortDescription, description, image, light, dates, customDate, durationInMinutes, maxSubscribers,
				subscribers, costMember, costNonMember, waitingList, maxWaitingList, location, bookingTemplate, waitingTemplate, externalOperator);
	}

	public abstract String id();

	@Nullable
	public abstract String sheetId();

	@Nullable
	public abstract Long gid();

	@Nullable
	public abstract EventType type();

	@Nullable
	public abstract String name();

	@Nullable
	public abstract Long sortIndex();

	@Nullable
	public abstract Boolean visible();

	@Nullable
	public abstract Boolean beta();

	@Nullable
	public abstract String shortDescription();

	@Nullable
	public abstract String description();

	@Nullable
	public abstract String image();

	@Nullable
	public abstract Boolean light();

	@Nullable
	public abstract List<LocalDateTime> dates();

	@Nullable
	public abstract String customDate();

	@Nullable
	public abstract Long durationInMinutes();

	@Nullable
	public abstract Long maxSubscribers();

	@Nullable
	public abstract Long subscribers();

	@Nullable
	public abstract Double costMember();

	@Nullable
	public abstract Double costNonMember();

	@Nullable
	public abstract Long waitingList();

	@Nullable
	public abstract Long maxWaitingList();

	@Nullable
	public abstract String location();

	@Nullable
	public abstract String bookingTemplate();

	@Nullable
	public abstract String waitingTemplate();

	@Nullable
	public abstract Boolean externalOperator();

	public boolean isBookedUp() {
		if (maxSubscribers() == -1) {
			return false;
		}
		return subscribers() >= maxSubscribers() && waitingList() >= maxWaitingList();
	}

	public Event update(Event event) {
		return create(id(),
					  requireNonNullElse(event.sheetId(), sheetId()),
					  requireNonNullElse(event.gid(), gid()),
					  requireNonNullElse(event.type(), type()),
					  requireNonNullElse(event.name(), name()),
					  requireNonNullElse(event.sortIndex(), sortIndex()),
					  requireNonNullElse(event.visible(), visible()),
					  requireNonNullElse(event.beta(), beta()),
					  requireNonNullElse(event.shortDescription(), shortDescription()),
					  requireNonNullElse(event.description(), description()),
					  requireNonNullElse(event.image(), image()),
					  requireNonNullElse(event.light(), light()),
					  requireNonNullElse(event.dates(), dates()),
					  Optional.ofNullable(event.customDate()).orElse(customDate()),
					  requireNonNullElse(event.durationInMinutes(), durationInMinutes()),
					  requireNonNullElse(event.maxSubscribers(), maxSubscribers()),
					  requireNonNullElse(event.subscribers(), subscribers()),
					  requireNonNullElse(event.costMember(), costMember()),
					  requireNonNullElse(event.costNonMember(), costNonMember()),
					  requireNonNullElse(event.waitingList(), waitingList()),
					  requireNonNullElse(event.maxWaitingList(), maxWaitingList()),
					  requireNonNullElse(event.location(), location()),
					  requireNonNullElse(event.bookingTemplate(), bookingTemplate()),
					  requireNonNullElse(event.waitingTemplate(), waitingTemplate()),
					  requireNonNullElse(event.externalOperator(), externalOperator()));
	}

}
